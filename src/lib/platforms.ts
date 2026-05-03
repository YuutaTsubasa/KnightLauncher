import mdSymbol from '../assets/platforms/md.png';
import satSymbol from '../assets/platforms/sat.png';
import dcSymbol from '../assets/platforms/DC.png';
import dsSymbol from '../assets/platforms/ds.png';
import threedsSymbol from '../assets/platforms/3ds.png';
import wiiSymbol from '../assets/platforms/wii.png';
import wiiuSymbol from '../assets/platforms/wiiu.png';

export type PlatformIcon =
  | { kind: 'svg'; viewBox: string; path: string }
  | { kind: 'png'; src: string };

export type Platform = {
  id: string;
  label: string;
  color1: string;
  color2: string;
  icon: PlatformIcon;
};

export const PLATFORMS: Platform[] = [
  {
    id: 'steam',
    label: 'Steam',
    color1: '#1B2838',
    color2: '#66C0F4',
    icon: {
      kind: 'svg',
      viewBox: '0 0 24 24',
      path: 'M11.979 0C5.678 0 .511 4.86.022 11.037l6.432 2.658c.545-.371 1.203-.59 1.912-.59.063 0 .125.004.188.006l2.861-4.142V8.91c0-2.495 2.028-4.524 4.524-4.524 2.494 0 4.524 2.031 4.524 4.527s-2.03 4.525-4.524 4.525h-.105l-4.076 2.911c0 .052.004.105.004.159 0 1.875-1.515 3.396-3.39 3.396-1.635 0-3.016-1.173-3.331-2.727L.436 15.27C1.862 20.307 6.486 24 11.979 24c6.627 0 11.999-5.373 11.999-12S18.605 0 11.979 0zM7.54 18.21l-1.473-.61c.262.543.714.999 1.314 1.25 1.297.539 2.793-.076 3.332-1.375.263-.63.264-1.319.005-1.949s-.75-1.121-1.377-1.383c-.624-.26-1.29-.249-1.878-.03l1.523.63c.956.4 1.409 1.5 1.009 2.455-.397.957-1.497 1.41-2.454 1.012H7.54zm11.415-9.303c0-1.662-1.353-3.015-3.015-3.015-1.665 0-3.015 1.353-3.015 3.015 0 1.665 1.35 3.015 3.015 3.015 1.663 0 3.015-1.35 3.015-3.015zm-5.273-.005c0-1.252 1.013-2.266 2.265-2.266 1.249 0 2.266 1.014 2.266 2.266 0 1.251-1.017 2.265-2.266 2.265-1.253 0-2.265-1.014-2.265-2.265z'
    }
  },
  {
    id: 'windows',
    label: 'Windows',
    color1: '#0078D4',
    color2: '#00BCF2',
    icon: {
      kind: 'svg',
      viewBox: '0 0 24 24',
      path: 'M0 3.449L9.75 2.1v9.451H0m10.949-9.602L24 0v11.4H10.949M0 12.6h9.75v9.451L0 20.699M10.949 12.6H24V24l-13.051-1.351'
    }
  },
  {
    id: 'md',
    label: 'Mega Drive / Genesis',
    color1: '#5C3F8E',
    color2: '#B83A66',
    icon: { kind: 'png', src: mdSymbol }
  },
  {
    id: 'sat',
    label: 'Saturn',
    color1: '#9281B0',
    color2: '#C28E9C',
    icon: { kind: 'png', src: satSymbol }
  },
  {
    id: 'dc',
    label: 'Dreamcast',
    color1: '#FF8533',
    color2: '#E85D00',
    icon: { kind: 'png', src: dcSymbol }
  },
  {
    id: 'ds',
    label: 'Nintendo DS',
    color1: '#E89BB5',
    color2: '#B6C9DB',
    icon: { kind: 'png', src: dsSymbol }
  },
  {
    id: '3ds',
    label: 'Nintendo 3DS',
    color1: '#FCB147',
    color2: '#F58A75',
    icon: { kind: 'png', src: threedsSymbol }
  },
  {
    id: 'wii',
    label: 'Wii',
    color1: '#7FCEDF',
    color2: '#BCEEF7',
    icon: { kind: 'png', src: wiiSymbol }
  },
  {
    id: 'wiiu',
    label: 'Wii U',
    color1: '#4FB6E0',
    color2: '#B7DCAB',
    icon: { kind: 'png', src: wiiuSymbol }
  },
  {
    id: 'other',
    label: 'Other',
    color1: '#4B5563',
    color2: '#9CA3AF',
    icon: {
      kind: 'svg',
      viewBox: '0 0 24 24',
      path: 'M3 5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2zM8 21h8M12 17v4'
    }
  }
];

const platformById = new Map(PLATFORMS.map((platform) => [platform.id, platform]));

const aliasMap: Record<string, string> = {
  steam: 'steam',
  valve: 'steam',
  windows: 'windows',
  win: 'windows',
  pc: 'windows',
  'microsoft windows': 'windows',
  md: 'md',
  smd: 'md',
  genesis: 'md',
  megadrive: 'md',
  'mega drive': 'md',
  'mega drive / genesis': 'md',
  sat: 'sat',
  saturn: 'sat',
  'sega saturn': 'sat',
  dc: 'dc',
  dreamcast: 'dc',
  'sega dreamcast': 'dc',
  ds: 'ds',
  nds: 'ds',
  'nintendo ds': 'ds',
  '3ds': '3ds',
  n3ds: '3ds',
  'nintendo 3ds': '3ds',
  wii: 'wii',
  'nintendo wii': 'wii',
  wiiu: 'wiiu',
  'wii u': 'wiiu',
  'nintendo wii u': 'wiiu'
};

export function resolvePlatform(name: string | null | undefined): Platform {
  if (!name) return platformById.get('other')!;
  const normalized = name.trim().toLowerCase();
  const id = aliasMap[normalized] ?? normalized;
  return platformById.get(id) ?? platformById.get('other')!;
}

export function frameGradient(platform: Platform): string {
  return `linear-gradient(135deg, ${platform.color1} 0%, ${platform.color1} 50%, ${platform.color2} 100%)`;
}
