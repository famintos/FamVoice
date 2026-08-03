[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$ManifestPath,

    [switch]$ExecutePaidCalls,

    [ValidateRange(0, 5)]
    [int]$MaxRetries = 1,

    [ValidateRange(10, 900)]
    [int]$RequestTimeoutSeconds = 120
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$maximumAudioBytes = 25MB
$maximumResponseChars = 1024 * 1024
$priceDate = '2026-08-02'
$supportedAudio = @{
    '.m4a'  = 'audio/mp4'
    '.mp3'  = 'audio/mpeg'
    '.mp4'  = 'video/mp4'
    '.mpeg' = 'audio/mpeg'
    '.mpga' = 'audio/mpeg'
    '.wav'  = 'audio/wav'
    '.webm' = 'audio/webm'
}

function Test-IsInsideRepo {
    param([Parameter(Mandatory = $true)][string]$CandidatePath)

    $candidate = [System.IO.Path]::GetFullPath($CandidatePath)
    $rootWithSeparator = $repoRoot.TrimEnd(
        [System.IO.Path]::DirectorySeparatorChar,
        [System.IO.Path]::AltDirectorySeparatorChar
    ) + [System.IO.Path]::DirectorySeparatorChar

    return $candidate.Equals($repoRoot, [System.StringComparison]::OrdinalIgnoreCase) -or
        $candidate.StartsWith($rootWithSeparator, [System.StringComparison]::OrdinalIgnoreCase)
}

function Resolve-PrivateFile {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Label
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "$Label was not found."
    }

    $resolved = (Resolve-Path -LiteralPath $Path).ProviderPath
    if (Test-IsInsideRepo -CandidatePath $resolved) {
        throw "$Label must be stored outside the FamVoice repository."
    }

    return $resolved
}

function Get-JsonValue {
    param(
        [Parameter(Mandatory = $true)][object]$Object,
        [Parameter(Mandatory = $true)][string]$Name
    )

    $property = $Object.PSObject.Properties[$Name]
    if ($null -eq $property) {
        return $null
    }
    return $property.Value
}

function Normalize-Transcript {
    param([AllowEmptyString()][string]$Text)

    if ([string]::IsNullOrWhiteSpace($Text)) {
        return ''
    }

    $normalized = $Text.Normalize([System.Text.NormalizationForm]::FormC).ToLowerInvariant()
    $normalized = $normalized -replace '[\u2018\u2019]', "'"
    $normalized = $normalized -replace "[^\p{L}\p{Nd}']+", ' '
    return ($normalized -replace '\s+', ' ').Trim()
}

function Get-EditDistance {
    param(
        [AllowEmptyCollection()][object[]]$Reference,
        [AllowEmptyCollection()][object[]]$Hypothesis
    )

    $previous = [int[]]::new($Hypothesis.Count + 1)
    for ($column = 0; $column -le $Hypothesis.Count; $column++) {
        $previous[$column] = $column
    }

    for ($row = 1; $row -le $Reference.Count; $row++) {
        $current = [int[]]::new($Hypothesis.Count + 1)
        $current[0] = $row
        for ($column = 1; $column -le $Hypothesis.Count; $column++) {
            $substitutionCost = if ($Reference[$row - 1] -ceq $Hypothesis[$column - 1]) { 0 } else { 1 }
            $current[$column] = [Math]::Min(
                [Math]::Min($current[$column - 1] + 1, $previous[$column] + 1),
                $previous[$column - 1] + $substitutionCost
            )
        }
        $previous = $current
    }

    return $previous[$Hypothesis.Count]
}

function Test-ExactTerm {
    param(
        [AllowEmptyCollection()][string[]]$TranscriptWords,
        [Parameter(Mandatory = $true)][string]$Term
    )

    $normalizedTerm = Normalize-Transcript -Text $Term
    if (-not $normalizedTerm) {
        return $false
    }

    [string[]]$termWords = @($normalizedTerm -split ' ')
    if ($termWords.Count -gt $TranscriptWords.Count) {
        return $false
    }

    for ($start = 0; $start -le $TranscriptWords.Count - $termWords.Count; $start++) {
        $matches = $true
        for ($offset = 0; $offset -lt $termWords.Count; $offset++) {
            if ($TranscriptWords[$start + $offset] -cne $termWords[$offset]) {
                $matches = $false
                break
            }
        }
        if ($matches) {
            return $true
        }
    }

    return $false
}

function Get-Percentile {
    param(
        [AllowEmptyCollection()][double[]]$Values,
        [ValidateRange(0, 1)][double]$Percentile
    )

    if ($Values.Count -eq 0) {
        return $null
    }

    [double[]]$sorted = @($Values | Sort-Object)
    if ($sorted.Count -eq 1) {
        return [Math]::Round($sorted[0], 1)
    }

    $position = ($sorted.Count - 1) * $Percentile
    $lower = [Math]::Floor($position)
    $upper = [Math]::Ceiling($position)
    if ($lower -eq $upper) {
        return [Math]::Round($sorted[[int]$lower], 1)
    }

    $weight = $position - $lower
    $value = $sorted[[int]$lower] + (($sorted[[int]$upper] - $sorted[[int]$lower]) * $weight)
    return [Math]::Round($value, 1)
}

function Add-MultipartText {
    param(
        [Parameter(Mandatory = $true)][System.Net.Http.MultipartFormDataContent]$Form,
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][string]$Value
    )

    $content = [System.Net.Http.StringContent]::new($Value, [System.Text.Encoding]::UTF8)
    $Form.Add($content, $Name)
}

function New-TranscriptionRequest {
    param(
        [Parameter(Mandatory = $true)][object]$Variant,
        [Parameter(Mandatory = $true)][object]$Sample,
        [Parameter(Mandatory = $true)][string]$ApiKey
    )

    $request = [System.Net.Http.HttpRequestMessage]::new(
        [System.Net.Http.HttpMethod]::Post,
        $Variant.Endpoint
    )
    $request.Headers.Authorization = [System.Net.Http.Headers.AuthenticationHeaderValue]::new(
        'Bearer',
        $ApiKey
    )

    $form = [System.Net.Http.MultipartFormDataContent]::new()
    $audioBytes = [System.IO.File]::ReadAllBytes($Sample.AudioPath)
    $audioContent = [System.Net.Http.ByteArrayContent]::new($audioBytes)
    $audioContent.Headers.ContentType = [System.Net.Http.Headers.MediaTypeHeaderValue]::new($Sample.MimeType)
    $form.Add($audioContent, 'file', [System.IO.Path]::GetFileName($Sample.AudioPath))
    Add-MultipartText -Form $form -Name 'model' -Value $Variant.Model
    Add-MultipartText -Form $form -Name 'response_format' -Value 'json'
    Add-MultipartText -Form $form -Name 'temperature' -Value '0'

    if ($Variant.Language) {
        Add-MultipartText -Form $form -Name 'language' -Value $Variant.Language
    }
    foreach ($language in @($Variant.Languages)) {
        Add-MultipartText -Form $form -Name 'languages[]' -Value $language
    }
    foreach ($keyword in @($Variant.Keywords)) {
        Add-MultipartText -Form $form -Name 'keywords[]' -Value $keyword
    }
    if ($Variant.Prompt) {
        Add-MultipartText -Form $form -Name 'prompt' -Value $Variant.Prompt
    }

    $request.Content = $form
    return $request
}

function Invoke-Transcription {
    param(
        [Parameter(Mandatory = $true)][System.Net.Http.HttpClient]$Client,
        [Parameter(Mandatory = $true)][object]$Variant,
        [Parameter(Mandatory = $true)][object]$Sample,
        [Parameter(Mandatory = $true)][string]$ApiKey
    )

    $timer = [System.Diagnostics.Stopwatch]::StartNew()
    $retries = 0
    $attempt = 0

    while ($true) {
        $attempt++
        $request = $null
        $response = $null
        $shouldRetry = $false
        try {
            $request = New-TranscriptionRequest -Variant $Variant -Sample $Sample -ApiKey $ApiKey
            $response = $Client.SendAsync($request).GetAwaiter().GetResult()
            $statusCode = [int]$response.StatusCode
            if (-not $response.IsSuccessStatusCode) {
                $shouldRetry = $statusCode -in @(408, 429, 500, 502, 503, 504)
                if (-not $shouldRetry -or $attempt -gt $MaxRetries) {
                    $timer.Stop()
                    return [pscustomobject]@{
                        Success = $false
                        Text = ''
                        LatencyMs = $timer.Elapsed.TotalMilliseconds
                        Retries = $retries
                    }
                }
            } else {
                $contentLength = $response.Content.Headers.ContentLength
                if ($null -ne $contentLength -and $contentLength -gt $maximumResponseChars) {
                    $timer.Stop()
                    return [pscustomobject]@{
                        Success = $false
                        Text = ''
                        LatencyMs = $timer.Elapsed.TotalMilliseconds
                        Retries = $retries
                    }
                }

                $body = $response.Content.ReadAsStringAsync().GetAwaiter().GetResult()
                if ($body.Length -gt $maximumResponseChars) {
                    $timer.Stop()
                    return [pscustomobject]@{
                        Success = $false
                        Text = ''
                        LatencyMs = $timer.Elapsed.TotalMilliseconds
                        Retries = $retries
                    }
                }

                try {
                    $payload = $body | ConvertFrom-Json
                    $text = [string](Get-JsonValue -Object $payload -Name 'text')
                } catch {
                    $text = ''
                }

                $timer.Stop()
                return [pscustomobject]@{
                    Success = -not [string]::IsNullOrWhiteSpace($text)
                    Text = $text
                    LatencyMs = $timer.Elapsed.TotalMilliseconds
                    Retries = $retries
                }
            }
        } catch [System.Threading.Tasks.TaskCanceledException] {
            $shouldRetry = $true
        } catch [System.Net.Http.HttpRequestException] {
            $shouldRetry = $true
        } catch {
            $timer.Stop()
            return [pscustomobject]@{
                Success = $false
                Text = ''
                LatencyMs = $timer.Elapsed.TotalMilliseconds
                Retries = $retries
            }
        } finally {
            if ($null -ne $response) {
                $response.Dispose()
            }
            if ($null -ne $request) {
                $request.Dispose()
            }
        }

        if (-not $shouldRetry -or $attempt -gt $MaxRetries) {
            $timer.Stop()
            return [pscustomobject]@{
                Success = $false
                Text = ''
                LatencyMs = $timer.Elapsed.TotalMilliseconds
                Retries = $retries
            }
        }

        $retries++
        Start-Sleep -Milliseconds ([Math]::Min(8000, 1000 * [Math]::Pow(2, $retries - 1)))
    }
}

function Get-EstimatedCost {
    param(
        [Parameter(Mandatory = $true)][string]$Provider,
        [Parameter(Mandatory = $true)][string]$Model,
        [Parameter(Mandatory = $true)][object[]]$Samples
    )

    if ($Provider -eq 'groq') {
        $billedSeconds = ($Samples | ForEach-Object { [Math]::Max(10.0, $_.DurationSeconds) } |
            Measure-Object -Sum).Sum
        return [Math]::Round(($billedSeconds / 3600.0) * 0.111, 6)
    }

    $minutes = (($Samples | Measure-Object -Property DurationSeconds -Sum).Sum) / 60.0
    $rate = if ($Model -eq 'whisper-1') { 0.006 } else { 0.0045 }
    return [Math]::Round($minutes * $rate, 6)
}

$resolvedManifestPath = Resolve-PrivateFile -Path $ManifestPath -Label 'The evaluation manifest'
try {
    $manifest = Get-Content -LiteralPath $resolvedManifestPath -Raw | ConvertFrom-Json
} catch {
    throw 'The private evaluation manifest is not valid JSON.'
}

$manifestVersion = Get-JsonValue -Object $manifest -Name 'version'
if ([int]$manifestVersion -ne 1) {
    throw 'The private evaluation manifest must use version 1.'
}

$language = ([string](Get-JsonValue -Object $manifest -Name 'language')).Trim().ToLowerInvariant()
if ($language -notin @('pt', 'pt-pt')) {
    throw "This harness only accepts a pt-PT manifest (language 'pt' or 'pt-PT')."
}
$apiLanguage = 'pt'

$prompt = ([string](Get-JsonValue -Object $manifest -Name 'prompt')).Trim()
if ([string]::IsNullOrWhiteSpace($prompt)) {
    throw 'The manifest must provide a non-empty prompt for the contextual gpt-transcribe variant.'
}
if ($prompt.Length -gt 800) {
    throw 'The manifest prompt must not exceed 800 characters.'
}

[string[]]$keywords = @(
    @(Get-JsonValue -Object $manifest -Name 'keywords') |
        ForEach-Object { ([string]$_).Trim() } |
        Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
)
if ($keywords.Count -eq 0) {
    throw 'The manifest must provide at least one keyword for the contextual gpt-transcribe variant.'
}
if ($keywords.Count -gt 20) {
    throw 'The manifest must not provide more than 20 keywords.'
}
foreach ($keyword in $keywords) {
    if ($keyword -match '[<>\r\n]') {
        throw 'Manifest keywords cannot contain angle brackets or line breaks.'
    }
}

$manifestDirectory = Split-Path -Parent $resolvedManifestPath
$rawSamples = @(Get-JsonValue -Object $manifest -Name 'samples')
if ($rawSamples.Count -eq 0) {
    throw 'The manifest must contain at least one sample.'
}

$samples = [System.Collections.Generic.List[object]]::new()
$sampleNumber = 0
foreach ($rawSample in $rawSamples) {
    $sampleNumber++
    $audioPath = ([string](Get-JsonValue -Object $rawSample -Name 'audioPath')).Trim()
    if ([string]::IsNullOrWhiteSpace($audioPath)) {
        throw "Sample $sampleNumber has no audioPath."
    }
    if (-not [System.IO.Path]::IsPathRooted($audioPath)) {
        $audioPath = Join-Path $manifestDirectory $audioPath
    }
    $resolvedAudioPath = Resolve-PrivateFile -Path $audioPath -Label "Audio for sample $sampleNumber"
    $audioItem = Get-Item -LiteralPath $resolvedAudioPath
    if ($audioItem.Length -le 0 -or $audioItem.Length -gt $maximumAudioBytes) {
        throw "Sample $sampleNumber audio must be non-empty and no larger than 25 MB."
    }

    $extension = [System.IO.Path]::GetExtension($resolvedAudioPath).ToLowerInvariant()
    if (-not $supportedAudio.ContainsKey($extension)) {
        throw "Sample $sampleNumber uses an unsupported audio format."
    }

    $reference = ([string](Get-JsonValue -Object $rawSample -Name 'reference')).Trim()
    $normalizedReference = Normalize-Transcript -Text $reference
    if ([string]::IsNullOrWhiteSpace($normalizedReference)) {
        throw "Sample $sampleNumber has no usable reference transcript."
    }

    $durationValue = Get-JsonValue -Object $rawSample -Name 'durationSeconds'
    $durationSeconds = 0.0
    if ($null -eq $durationValue -or
        -not [double]::TryParse(
            [string]$durationValue,
            [System.Globalization.NumberStyles]::Float,
            [System.Globalization.CultureInfo]::InvariantCulture,
            [ref]$durationSeconds
        ) -or
        $durationSeconds -le 0) {
        throw "Sample $sampleNumber must have a positive durationSeconds value."
    }

    [string[]]$terms = @(
        @(Get-JsonValue -Object $rawSample -Name 'terms') |
            ForEach-Object { ([string]$_).Trim() } |
            Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
    )
    $normalizedTerms = @($terms | ForEach-Object { Normalize-Transcript -Text $_ })
    if (@($normalizedTerms | Sort-Object -Unique).Count -ne $normalizedTerms.Count) {
        throw "Sample $sampleNumber contains duplicate terms after normalization."
    }
    [string[]]$unspokenKeywords = @(
        $keywords | Where-Object {
            $normalizedKeyword = Normalize-Transcript -Text $_
            $normalizedKeyword -notin $normalizedTerms
        }
    )

    $samples.Add([pscustomobject]@{
        AudioPath = $resolvedAudioPath
        MimeType = $supportedAudio[$extension]
        DurationSeconds = $durationSeconds
        Reference = $normalizedReference
        Terms = $terms
        UnspokenKeywords = $unspokenKeywords
    })
}

$variants = @(
    [pscustomobject]@{
        Id = 'groq-whisper-large-v3-pt'
        Provider = 'groq'
        Model = 'whisper-large-v3'
        Endpoint = 'https://api.groq.com/openai/v1/audio/transcriptions'
        Language = $apiLanguage
        Languages = @()
        Keywords = @()
        Prompt = ''
    },
    [pscustomobject]@{
        Id = 'openai-whisper-1-pt'
        Provider = 'openai'
        Model = 'whisper-1'
        Endpoint = 'https://api.openai.com/v1/audio/transcriptions'
        Language = $apiLanguage
        Languages = @()
        Keywords = @()
        Prompt = ''
    },
    [pscustomobject]@{
        Id = 'openai-gpt-transcribe-no-context'
        Provider = 'openai'
        Model = 'gpt-transcribe'
        Endpoint = 'https://api.openai.com/v1/audio/transcriptions'
        Language = ''
        Languages = @()
        Keywords = @()
        Prompt = ''
    },
    [pscustomobject]@{
        Id = 'openai-gpt-transcribe-pt-context'
        Provider = 'openai'
        Model = 'gpt-transcribe'
        Endpoint = 'https://api.openai.com/v1/audio/transcriptions'
        Language = ''
        Languages = @($apiLanguage)
        Keywords = @($keywords)
        Prompt = $prompt
    }
)

$results = [System.Collections.Generic.List[object]]::new()

if (-not $ExecutePaidCalls) {
    foreach ($variant in $variants) {
        $results.Add([pscustomobject]@{
            Variant = $variant.Id
            Measurement = 'not measured'
            SampleCount = $samples.Count
            SuccessCount = 'not measured'
            FailureCount = 'not measured'
            RetryCount = 'not measured'
            WerPercent = 'not measured'
            CerPercent = 'not measured'
            ExactTermAccuracyPercent = 'not measured'
            UnspokenKeywordOccurrencePercent = 'not measured'
            FinalLatencyP50Ms = 'not measured'
            FinalLatencyP95Ms = 'not measured'
            EstimatedCostUsd = Get-EstimatedCost -Provider $variant.Provider -Model $variant.Model -Samples $samples
        })
    }
} else {
    $openAiKey = [Environment]::GetEnvironmentVariable('OPENAI_API_KEY')
    $groqKey = [Environment]::GetEnvironmentVariable('GROQ_API_KEY')
    if ([string]::IsNullOrWhiteSpace($openAiKey)) {
        throw 'OPENAI_API_KEY is required when -ExecutePaidCalls is set.'
    }
    if ([string]::IsNullOrWhiteSpace($groqKey)) {
        throw 'GROQ_API_KEY is required when -ExecutePaidCalls is set.'
    }

    Add-Type -AssemblyName System.Net.Http
    $client = [System.Net.Http.HttpClient]::new()
    $client.Timeout = [TimeSpan]::FromSeconds($RequestTimeoutSeconds)
    try {
        foreach ($variant in $variants) {
            $wordEdits = 0
            $referenceWords = 0
            $characterEdits = 0
            $referenceCharacters = 0
            $termHits = 0
            $termTotal = 0
            $unspokenKeywordOccurrences = 0
            $unspokenKeywordChecks = 0
            $successes = 0
            $failures = 0
            $retryTotal = 0
            $latencies = [System.Collections.Generic.List[double]]::new()

            $apiKey = if ($variant.Provider -eq 'groq') { $groqKey } else { $openAiKey }
            foreach ($sample in $samples) {
                $call = Invoke-Transcription -Client $client -Variant $variant -Sample $sample -ApiKey $apiKey
                $latencies.Add($call.LatencyMs)
                $retryTotal += $call.Retries

                $hypothesis = if ($call.Success) { Normalize-Transcript -Text $call.Text } else { '' }
                if ($call.Success -and $hypothesis) {
                    $successes++
                } else {
                    $failures++
                    $hypothesis = ''
                }

                [string[]]$referenceWordArray = @($sample.Reference -split ' ')
                [string[]]$hypothesisWordArray = if ($hypothesis) { @($hypothesis -split ' ') } else { @() }
                $wordEdits += Get-EditDistance -Reference $referenceWordArray -Hypothesis $hypothesisWordArray
                $referenceWords += $referenceWordArray.Count

                [object[]]$referenceCharacterArray = @($sample.Reference.ToCharArray())
                [object[]]$hypothesisCharacterArray = if ($hypothesis) { @($hypothesis.ToCharArray()) } else { @() }
                $characterEdits += Get-EditDistance -Reference $referenceCharacterArray -Hypothesis $hypothesisCharacterArray
                $referenceCharacters += $referenceCharacterArray.Count

                foreach ($term in $sample.Terms) {
                    $termTotal++
                    if (Test-ExactTerm -TranscriptWords $hypothesisWordArray -Term $term) {
                        $termHits++
                    }
                }
                foreach ($keyword in $sample.UnspokenKeywords) {
                    $unspokenKeywordChecks++
                    if (Test-ExactTerm -TranscriptWords $hypothesisWordArray -Term $keyword) {
                        $unspokenKeywordOccurrences++
                    }
                }
            }

            $termAccuracy = if ($termTotal -gt 0) {
                [Math]::Round(($termHits / $termTotal) * 100.0, 2)
            } else {
                'not measured'
            }
            $unspokenKeywordOccurrenceRate = if ($unspokenKeywordChecks -gt 0) {
                [Math]::Round(($unspokenKeywordOccurrences / $unspokenKeywordChecks) * 100.0, 2)
            } else {
                'not measured'
            }

            $results.Add([pscustomobject]@{
                Variant = $variant.Id
                Measurement = 'measured'
                SampleCount = $samples.Count
                SuccessCount = $successes
                FailureCount = $failures
                RetryCount = $retryTotal
                WerPercent = [Math]::Round(($wordEdits / $referenceWords) * 100.0, 2)
                CerPercent = [Math]::Round(($characterEdits / $referenceCharacters) * 100.0, 2)
                ExactTermAccuracyPercent = $termAccuracy
                UnspokenKeywordOccurrencePercent = $unspokenKeywordOccurrenceRate
                FinalLatencyP50Ms = Get-Percentile -Values $latencies -Percentile 0.50
                FinalLatencyP95Ms = Get-Percentile -Values $latencies -Percentile 0.95
                EstimatedCostUsd = Get-EstimatedCost -Provider $variant.Provider -Model $variant.Model -Samples $samples
            })
        }
    } finally {
        $client.Dispose()
        $openAiKey = $null
        $groqKey = $null
    }
}

$estimatedTotal = [Math]::Round((($results | Measure-Object -Property EstimatedCostUsd -Sum).Sum), 6)
$report = [ordered]@{
    Harness = 'FamVoice Phase 5 pt-PT transcription evaluation'
    Mode = if ($ExecutePaidCalls) { 'paid calls executed' } else { 'preflight only - no API calls made' }
    MeasurementDateUtc = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
    PricingAsOf = $priceDate
    Corpus = [ordered]@{
        SampleCount = $samples.Count
        TotalDurationSeconds = [Math]::Round((($samples | Measure-Object -Property DurationSeconds -Sum).Sum), 3)
        DeclaredTermCount = (($samples | ForEach-Object { $_.Terms.Count } | Measure-Object -Sum).Sum)
    }
    EstimatedCorpusPassCostUsd = $estimatedTotal
    Results = @($results)
    RatesUsd = [ordered]@{
        GptTranscribePerMinute = 0.0045
        Whisper1PerMinute = 0.006
        GroqWhisperLargeV3PerHour = 0.111
        GroqMinimumBilledSecondsPerRequest = 10
    }
    PricingSources = @(
        'https://developers.openai.com/api/docs/models/gpt-transcribe',
        'https://developers.openai.com/api/docs/models/whisper-1',
        'https://console.groq.com/docs/speech-to-text'
    )
    Privacy = 'Aggregate metrics only. No audio, references, transcripts, API keys, manifest paths, or sample paths are emitted or persisted by this harness.'
}

$report | ConvertTo-Json -Depth 6
