export function VoiceWave({
  isPlaying = false,
  size = "default",
}: {
  isPlaying?: boolean;
  size?: "default" | "large";
}) {
  const containerClass = size === "large"
    ? "h-8 gap-1"
    : "h-4 gap-[2px]";
  const barClass = size === "large" ? "w-[4px]" : "w-[3px]";
  const pausedHeight = size === "large" ? "55%" : "40%";

  return (
    <div className={`flex items-center justify-center ${containerClass} pointer-events-none`}>
      {[...Array(5)].map((_, i) => (
        <div
          key={i}
          className={`wave-bar ${barClass} bg-primary rounded-full h-full ${!isPlaying ? "pause-animation" : ""}`}
          style={{ height: isPlaying ? undefined : pausedHeight }}
        />
      ))}
    </div>
  );
}
