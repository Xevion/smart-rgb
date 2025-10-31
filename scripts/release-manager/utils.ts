import { S3Client } from '@aws-sdk/client-s3';
import { GetObjectCommand, PutObjectCommand } from '@aws-sdk/client-s3';
import { existsSync } from 'fs';
import { dirname, resolve } from 'path';
import { fileURLToPath } from 'url';
import type { Manifest } from './types.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

export function getRepoRoot(): string {
  let currentDir = __dirname;

  while (currentDir !== dirname(currentDir)) {
    if (existsSync(resolve(currentDir, '.git'))) {
      return currentDir;
    }
    currentDir = dirname(currentDir);
  }

  throw new Error('Could not find repository root (no .git directory found)');
}

export function getReleasesDir(): string {
  return resolve(getRepoRoot(), 'releases');
}

export function createR2Client(): S3Client {
  const accountId = process.env.R2_ACCOUNT_ID!;
  const accessKeyId = process.env.R2_ACCESS_KEY_ID!;
  const secretAccessKey = process.env.R2_SECRET_ACCESS_KEY!;

  return new S3Client({
    region: 'auto',
    endpoint: `https://${accountId}.r2.cloudflarestorage.com`,
    credentials: {
      accessKeyId,
      secretAccessKey,
    },
  });
}

export async function loadManifest(bucket: string, s3Client: S3Client): Promise<Manifest> {
  try {
    const response = await s3Client.send(
      new GetObjectCommand({
        Bucket: bucket,
        Key: 'manifest.json',
      })
    );

    const bodyString = await response.Body?.transformToString();
    if (!bodyString || bodyString.trim() === '') {
      throw new Error('manifest.json is empty');
    }

    const manifest = JSON.parse(bodyString);

    if (!manifest.versions || !Array.isArray(manifest.versions)) {
      throw new Error('Invalid manifest.json: missing or invalid "versions" array');
    }

    return manifest;
  } catch (error: any) {
    if (error.name === 'NoSuchKey' || error.Code === 'NoSuchKey') {
      throw new Error('manifest.json not found in R2 bucket. Upload a release first.');
    }

    if (error instanceof SyntaxError) {
      throw new Error(
        `Invalid JSON in manifest.json from bucket "${bucket}".\n` +
          `Please fix or delete the manifest and try again.`
      );
    }

    throw error;
  }
}

export async function createManifest(): Promise<Manifest> {
  return {
    latest: '',
    display_mode: 'latest_only',
    versions: [],
  };
}

export async function saveManifest(
  manifest: Manifest,
  bucket: string,
  s3Client: S3Client
): Promise<void> {
  try {
    const manifestJson = JSON.stringify(manifest, null, 2);

    await s3Client.send(
      new PutObjectCommand({
        Bucket: bucket,
        Key: 'manifest.json',
        Body: manifestJson,
        ContentType: 'application/json',
      })
    );
  } catch (error: any) {
    throw new Error(
      `Failed to upload manifest to R2 bucket "${bucket}": ${error.message}\n` +
        `Hint: Check that the bucket exists and your R2 credentials are correct`
    );
  }
}

function stripAnsi(str: string): number {
  return str.replace(/\u001B\[\d+m/g, '').length;
}

export function padWithColor(str: string, targetLength: number): string {
  const visibleLength = stripAnsi(str);
  const padding = Math.max(0, targetLength - visibleLength);
  return str + ' '.repeat(padding);
}

export function constrainString(str: string, maxLength: number): string {
  if (str.length <= maxLength) {
    return str;
  }

  const ratio = maxLength / str.length;

  if (ratio < 0.65) {
    const charsPerSide = Math.floor((maxLength - 1) / 2);
    const start = str.slice(0, charsPerSide);
    const end = str.slice(-charsPerSide);
    return `${start}…${end}`;
  } else {
    return str.slice(0, maxLength - 1) + '…';
  }
}

export function formatRelativeTime(date: Date): string {
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffHours = diffMs / (1000 * 60 * 60);

  if (diffHours > 12) {
    return date.toLocaleString();
  }

  const diffMins = Math.floor(diffMs / (1000 * 60));

  if (diffMins < 60) {
    return `${diffMins} min${diffMins !== 1 ? 's' : ''} ago`;
  }

  const hours = Math.floor(diffMins / 60);
  const mins = diffMins % 60;

  if (mins === 0) {
    return `${hours} hour${hours !== 1 ? 's' : ''} ago`;
  }

  return `${hours}h ${mins}m ago`;
}
