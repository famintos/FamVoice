import famintoMarkAmber from './assets/faminto-mark-amber.svg';

export const FamVoiceLogo = ({ size = 24, className = '' }: { size?: number | string, className?: string }) => (
  <img 
    src={famintoMarkAmber} 
    alt="FamVoice Logo" 
    width={size} 
    height={size} 
    className={className} 
    draggable={false} 
  />
);