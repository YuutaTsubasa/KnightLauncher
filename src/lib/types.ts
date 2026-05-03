export type GameVariant = {
  id: string;
  label: string;
  romPath: string;
  lastPlayedAt: string | null;
  playCount: number;
  retroAchievements: RetroAchievementsLink | null;
};

export type Achievement = {
  id: number;
  title: string;
  description: string;
  points: number;
  badgeUrl: string;
  badgeLockedUrl: string;
  badgePath: string | null;
  badgeLockedPath: string | null;
  earnedDate: string | null;
  displayOrder: number;
};

export type RetroAchievementsLink = {
  gameId: number;
  title: string;
  consoleId: number;
  consoleName: string;
  iconPath: string | null;
  iconUrl: string | null;
  boxArtUrl: string | null;
  titleUrl: string | null;
  ingameUrl: string | null;
  achievementsTotal: number;
  achievementsEarned: number;
  pointsTotal: number;
  pointsEarned: number;
  achievements: Achievement[];
  lastSyncedAt: string | null;
};

export type RaGameSearchResult = {
  id: number;
  title: string;
  consoleId: number;
  consoleName: string;
  iconUrl: string | null;
  numAchievements: number;
  points: number;
};

export type Game = {
  id: string;
  title: string;
  executablePath: string;
  launchArgs: string;
  workingDirectory: string;
  coverImage: string | null;
  heroImage: string | null;
  logoImage: string | null;
  favorite: boolean;
  lastPlayedAt: string | null;
  playCount: number;
  description: string | null;
  platform: string | null;
  tags: string[];
  romSystem: string | null;
  variants: GameVariant[];
  retroAchievements: RetroAchievementsLink | null;
  position: number;
  hidden: boolean;
};

export type GameLibrary = {
  games: Game[];
};

export type DisplayInfo = {
  id: number;
  name: string | null;
  x: number;
  y: number;
  width: number;
  height: number;
  scaleFactor: number;
};

export type LibraryFilter = 'all' | 'favorites' | 'recent' | 'hidden';

export type SortMode = 'title' | 'recent' | 'playCount' | 'manual';

export type AppSettings = {
  steamgriddbApiKey: string | null;
  googleApiKey: string | null;
  googleSearchEngineId: string | null;
  emudeckRoot: string | null;
  retroAchievementsUser: string | null;
  retroAchievementsApiKey: string | null;
};

export type SteamGridDbGame = {
  id: number;
  name: string;
  types: string[];
  verified: boolean;
};

export type SteamGridDbAsset = {
  id: number;
  kind: 'cover' | 'hero' | 'logo' | 'icon';
  url: string;
  thumb: string;
  width: number | null;
  height: number | null;
  style: string | null;
};

export type SteamGridDbArtwork = {
  covers: SteamGridDbAsset[];
  heroes: SteamGridDbAsset[];
  logos: SteamGridDbAsset[];
  icons: SteamGridDbAsset[];
};

export type GoogleImageResult = {
  title: string;
  link: string;
  thumbnail: string;
  contextLink: string | null;
  width: number | null;
  height: number | null;
  mime: string | null;
};
