export type Game = {
  id: string;
  title: string;
  executablePath: string;
  launchArgs: string;
  workingDirectory: string;
  coverImage: string | null;
  heroImage: string | null;
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
