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

export type LibraryFilter = 'all' | 'favorites' | 'recent';

export type SortMode = 'title' | 'recent' | 'playCount';

export type AppSettings = {
  steamgriddbApiKey: string | null;
  googleApiKey: string | null;
  googleSearchEngineId: string | null;
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
