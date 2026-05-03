import mdSymbol from '../assets/platforms/md.png';
import satSymbol from '../assets/platforms/sat.png';
import dcSymbol from '../assets/platforms/DC.png';
import dsSymbol from '../assets/platforms/ds.png';
import threedsSymbol from '../assets/platforms/3ds.png';
import wiiSymbol from '../assets/platforms/wii.png';
import wiiuSymbol from '../assets/platforms/wiiu.png';
import sfcSymbol from '../assets/platforms/sfc.png';
import fcSymbol from '../assets/platforms/fc.png';
import n64Symbol from '../assets/platforms/n64.png';
import gbSymbol from '../assets/platforms/gb.png';
import gbcSymbol from '../assets/platforms/gbc.png';
import gbaSymbol from '../assets/platforms/gba.png';
import gcSymbol from '../assets/platforms/gc.png';
import switchSymbol from '../assets/platforms/switch.png';
import ggSymbol from '../assets/platforms/GG.png';
import ngpcSymbol from '../assets/platforms/ngpc.png';
import psxSymbol from '../assets/platforms/psx.png';
import ps2Symbol from '../assets/platforms/ps2.png';
import pspSymbol from '../assets/platforms/psp.png';
import vitaSymbol from '../assets/platforms/vita.png';
import androidSymbol from '../assets/platforms/android.png';
import sega32xSymbol from '../assets/platforms/32x.png';

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
    id: 'fc',
    label: 'Famicom / NES',
    color1: '#E89357',
    color2: '#F0C062',
    icon: { kind: 'png', src: fcSymbol }
  },
  {
    id: 'sfc',
    label: 'Super Famicom / SNES',
    color1: '#D5407F',
    color2: '#A576B4',
    icon: { kind: 'png', src: sfcSymbol }
  },
  {
    id: 'n64',
    label: 'Nintendo 64',
    color1: '#5B7E47',
    color2: '#B85040',
    icon: { kind: 'png', src: n64Symbol }
  },
  {
    id: 'gc',
    label: 'GameCube',
    color1: '#7E72FF',
    color2: '#B5A7FF',
    icon: { kind: 'png', src: gcSymbol }
  },
  {
    id: 'gb',
    label: 'Game Boy',
    color1: '#9CC9A2',
    color2: '#B5D5C0',
    icon: { kind: 'png', src: gbSymbol }
  },
  {
    id: 'gbc',
    label: 'Game Boy Color',
    color1: '#D8C926',
    color2: '#B6CE3F',
    icon: { kind: 'png', src: gbcSymbol }
  },
  {
    id: 'gba',
    label: 'Game Boy Advance',
    color1: '#7176C8',
    color2: '#A5A8E2',
    icon: { kind: 'png', src: gbaSymbol }
  },
  {
    id: 'switch',
    label: 'Switch',
    color1: '#FF3030',
    color2: '#E04040',
    icon: { kind: 'png', src: switchSymbol }
  },
  {
    id: 'gg',
    label: 'Game Gear',
    color1: '#D5C700',
    color2: '#B6CD2D',
    icon: { kind: 'png', src: ggSymbol }
  },
  {
    id: 'ngpc',
    label: 'Neo Geo Pocket',
    color1: '#C97A78',
    color2: '#A6BB95',
    icon: { kind: 'png', src: ngpcSymbol }
  },
  {
    id: 'ps1',
    label: 'PlayStation',
    color1: '#A8A2D5',
    color2: '#B6B3DD',
    icon: { kind: 'png', src: psxSymbol }
  },
  {
    id: 'ps2',
    label: 'PlayStation 2',
    color1: '#5A47E0',
    color2: '#7E58E8',
    icon: { kind: 'png', src: ps2Symbol }
  },
  {
    id: 'psp',
    label: 'PSP',
    color1: '#D940DC',
    color2: '#A744D8',
    icon: { kind: 'png', src: pspSymbol }
  },
  {
    id: 'vita',
    label: 'PS Vita',
    color1: '#7E78E0',
    color2: '#5F8CDB',
    icon: { kind: 'png', src: vitaSymbol }
  },
  {
    id: 'android',
    label: 'Android',
    color1: '#3FD6A2',
    color2: '#85DDB6',
    icon: { kind: 'png', src: androidSymbol }
  },
  {
    id: '32x',
    label: 'Sega 32X',
    color1: '#B7202E',
    color2: '#E85742',
    icon: { kind: 'png', src: sega32xSymbol }
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
  'nintendo wii u': 'wiiu',
  fc: 'fc',
  famicom: 'fc',
  nes: 'fc',
  'nintendo entertainment system': 'fc',
  sfc: 'sfc',
  snes: 'sfc',
  'super famicom': 'sfc',
  'super nintendo': 'sfc',
  n64: 'n64',
  'nintendo 64': 'n64',
  gc: 'gc',
  gamecube: 'gc',
  'nintendo gamecube': 'gc',
  gb: 'gb',
  'game boy': 'gb',
  gameboy: 'gb',
  gbc: 'gbc',
  'game boy color': 'gbc',
  gameboycolor: 'gbc',
  gba: 'gba',
  'game boy advance': 'gba',
  gameboyadvance: 'gba',
  switch: 'switch',
  'nintendo switch': 'switch',
  gg: 'gg',
  gamegear: 'gg',
  'game gear': 'gg',
  'sega game gear': 'gg',
  ngp: 'ngpc',
  ngpc: 'ngpc',
  'neo geo pocket': 'ngpc',
  'neo geo pocket color': 'ngpc',
  ps1: 'ps1',
  psx: 'ps1',
  playstation: 'ps1',
  'playstation 1': 'ps1',
  ps2: 'ps2',
  'playstation 2': 'ps2',
  psp: 'psp',
  'playstation portable': 'psp',
  vita: 'vita',
  psvita: 'vita',
  'ps vita': 'vita',
  'playstation vita': 'vita',
  android: 'android',
  '32x': '32x',
  sega32x: '32x',
  'sega 32x': '32x',
  'genesis 32x': '32x',
  'mega drive 32x': '32x'
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
