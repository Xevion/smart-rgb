export interface Checksums {
  sha256: string;
  sha512: string;
  md5: string;
}

export type FileType = 'appimage' | 'deb' | 'rpm' | 'dmg' | 'app' | 'exe' | 'msi';

export interface ReleaseFile {
  type: FileType;
  url: string;
  filename: string;
  size: number;
  checksums: Checksums;
}

export type Platform =
  | 'linux-x86_64'
  | 'linux-aarch64'
  | 'macos-x86_64'
  | 'macos-aarch64'
  | 'windows-x86_64'
  | 'windows-aarch64';

export type PlatformFiles = {
  [K in Platform]?: {
    files: ReleaseFile[];
  };
};

export interface Version {
  version: string;
  released: string;
  visible: boolean;
  platforms: PlatformFiles;
}

export type DisplayMode = 'latest_only' | 'all' | 'recent_n';

export interface Manifest {
  latest: string;
  display_mode: DisplayMode;
  recent_n?: number;
  versions: Version[];
}

export interface ArtifactMapping {
  artifactName: string;
  platform: Platform;
  expectedFiles: {
    glob: string;
    type: FileType;
  }[];
}
