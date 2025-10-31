#!/usr/bin/env node
import { createHash } from "crypto";
import { mkdir, readFile, stat, unlink, writeFile } from "fs/promises";
import { basename, resolve } from "path";
import { program } from "commander";
import { glob } from "glob";
import { Octokit } from "@octokit/rest";
import { Upload } from "@aws-sdk/lib-storage";
import ora from "ora";
import pc from "picocolors";
import "dotenv/config";
import {
  createR2Client,
  getReleasesDir,
  loadManifest,
  createManifest,
  saveManifest,
  padWithColor,
  constrainString,
  formatRelativeTime,
} from "./utils.js";
import type {
  ArtifactMapping,
  Checksums,
  FileType,
  Manifest,
  Platform,
  ReleaseFile,
  Version,
} from "./types.js";

const ARTIFACT_MAPPINGS: ArtifactMapping[] = [
  {
    artifactName: "iron-borders-linux-x86_64",
    platform: "linux-x86_64",
    expectedFiles: [
      { glob: "**/*.AppImage", type: "appimage" },
      { glob: "**/*.deb", type: "deb" },
      { glob: "**/*.rpm", type: "rpm" },
    ],
  },
  {
    artifactName: "iron-borders-macos-x86_64",
    platform: "macos-x86_64",
    expectedFiles: [
      { glob: "**/*.dmg", type: "dmg" },
      { glob: "**/*.app", type: "app" },
    ],
  },
  {
    artifactName: "iron-borders-macos-aarch64",
    platform: "macos-aarch64",
    expectedFiles: [
      { glob: "**/*.dmg", type: "dmg" },
      { glob: "**/*.app", type: "app" },
    ],
  },
  {
    artifactName: "iron-borders-windows-x86_64",
    platform: "windows-x86_64",
    expectedFiles: [{ glob: "**/*.exe", type: "exe" }],
  },
  {
    artifactName: "iron-borders-windows-aarch64",
    platform: "windows-aarch64",
    expectedFiles: [{ glob: "**/*.exe", type: "exe" }],
  },
];

const artifactNames = ARTIFACT_MAPPINGS.map((m) => m.artifactName);

function validateEnvironment(): void {
  const required = [
    'GITHUB_TOKEN',
    'GITHUB_REPOSITORY',
    'R2_BUCKET_NAME',
    'R2_BASE_URL',
    'R2_ACCOUNT_ID',
    'R2_ACCESS_KEY_ID',
    'R2_SECRET_ACCESS_KEY',
  ];

  const missing = required.filter((key) => !process.env[key]);

  if (missing.length > 0) {
    console.error(pc.red('\n❌ Missing required environment variables:'));
    for (const key of missing) {
      console.error(pc.red(`  - ${key}`));
    }
    console.error(
      pc.yellow('\nCheck .env file in scripts/release-manager/.env\n')
    );
    process.exit(1);
  }
}

validateEnvironment();

async function downloadArtifact(
  octokit: Octokit,
  owner: string,
  repo: string,
  artifactId: number,
  outputPath: string
): Promise<void> {
  const response = await octokit.rest.actions.downloadArtifact({
    owner,
    repo,
    artifact_id: artifactId,
    archive_format: "zip",
  });

  const arrayBuffer = response.data as unknown as ArrayBuffer;
  const buffer = Buffer.from(arrayBuffer);

  await mkdir(resolve(outputPath, ".."), { recursive: true });
  await writeFile(outputPath, buffer);
}

async function extractZip(zipPath: string, outputDir: string): Promise<void> {
  const AdmZip = (await import("adm-zip")).default;
  const zip = new AdmZip(zipPath);
  zip.extractAllTo(outputDir, true);
}

async function listRecentRuns(
  octokit: Octokit,
  owner: string,
  repo: string
): Promise<void> {
  const workflows = await octokit.rest.actions.listRepoWorkflows({
    owner,
    repo,
  });

  const buildsWorkflow = workflows.data.workflows.find(
    (w) => w.path === ".github/workflows/builds.yml"
  );

  if (!buildsWorkflow) {
    throw new Error("builds.yml workflow not found");
  }

  const runs = await octokit.rest.actions.listWorkflowRuns({
    owner,
    repo,
    workflow_id: buildsWorkflow.id,
    per_page: 10,
  });

  console.log(pc.bold(pc.cyan("\n📋 Recent builds workflow runs:\n")));
  console.log(
    pc.dim(
      `${"Run ID".padEnd(12)} ${"Status".padEnd(12)} ${"Branch".padEnd(
        20
      )} ${"Created"}`
    )
  );
  console.log(pc.dim("─".repeat(80)));

  for (const run of runs.data.workflow_runs) {
    let status: string;
    if (run.status === "completed") {
      status =
        run.conclusion === "success"
          ? pc.green("✓ success")
          : pc.red(`✗ ${run.conclusion}`);
    } else {
      status = pc.yellow(`⋯ ${run.status}`);
    }

    const date = formatRelativeTime(new Date(run.created_at));
    const branch = constrainString(run.head_branch || "unknown", 20);

    console.log(
      `${String(run.id).padEnd(12)} ${padWithColor(status, 12)} ${branch.padEnd(
        20
      )} ${date}`
    );
  }

  console.log("");
}

async function computeChecksums(filePath: string): Promise<Checksums> {
  const buffer = await readFile(filePath);

  return {
    sha256: createHash("sha256").update(buffer).digest("hex"),
    sha512: createHash("sha512").update(buffer).digest("hex"),
    md5: createHash("md5").update(buffer).digest("hex"),
  };
}

async function findFiles(
  baseDir: string,
  artifactName: string,
  expectedFiles: { glob: string; type: FileType }[]
): Promise<{ path: string; type: FileType }[]> {
  const artifactDir = resolve(baseDir, artifactName);
  const results: { path: string; type: FileType }[] = [];

  for (const { glob: pattern, type } of expectedFiles) {
    const matches = await glob(pattern, {
      cwd: artifactDir,
      absolute: true,
      nodir: true,
    });

    for (const match of matches) {
      results.push({ path: match, type });
    }
  }

  return results;
}

async function uploadToR2(
  localPath: string,
  remotePath: string,
  bucket: string,
  s3Client: ReturnType<typeof createR2Client>,
  onProgress?: (percent: number) => void
): Promise<void> {
  try {
    const fileContent = await readFile(localPath);

    const upload = new Upload({
      client: s3Client,
      params: {
        Bucket: bucket,
        Key: remotePath,
        Body: fileContent,
      },
    });

    if (onProgress) {
      upload.on('httpUploadProgress', (progress) => {
        if (progress.loaded && progress.total) {
          const percent = Math.round((progress.loaded / progress.total) * 100);
          onProgress(percent);
        }
      });
    }

    await upload.done();
  } catch (error: any) {
    throw new Error(
      `Failed to upload to R2 bucket "${bucket}": ${error.message}\n` +
        `Path: ${remotePath}\n` +
        `Hint: Check that the bucket exists and your R2 credentials are correct`
    );
  }
}

async function processVersion(
  version: string,
  bucket: string,
  baseUrl: string,
  skipUpload: boolean,
  s3Client: ReturnType<typeof createR2Client>
): Promise<Version> {
  const releasesDir = getReleasesDir();
  const versionDir = resolve(releasesDir, version);

  const version_obj: Version = {
    version,
    released: new Date().toISOString(),
    visible: true,
    platforms: {},
  };

  for (const mapping of ARTIFACT_MAPPINGS) {
    const spinner = ora(`Finding files for ${mapping.platform}`).start();

    try {
      const files = await findFiles(
        versionDir,
        mapping.artifactName,
        mapping.expectedFiles
      );

      if (files.length === 0) {
        spinner.warn(pc.yellow(`No files found for ${mapping.platform}`));
        continue;
      }

      const releaseFiles: ReleaseFile[] = [];

      for (const { path: filePath, type } of files) {
        const filename = basename(filePath);
        const remotePath = `releases/v${version}/${mapping.platform}/${filename}`;

        spinner.text = `Computing checksums for ${filename}`;
        const checksums = await computeChecksums(filePath);
        const fileStats = await stat(filePath);

        if (!skipUpload) {
          spinner.text = `Uploading ${filename} (0%)`;
          await uploadToR2(filePath, remotePath, bucket, s3Client, (percent) => {
            spinner.text = `Uploading ${filename} (${percent}%)`;
          });
        }

        releaseFiles.push({
          type,
          url: `${baseUrl}/${remotePath}`,
          filename,
          size: fileStats.size,
          checksums,
        });
      }

      version_obj.platforms[mapping.platform] = { files: releaseFiles };
      spinner.succeed(
        pc.green(`Processed ${mapping.platform} (${releaseFiles.length} files)`)
      );
    } catch (error) {
      spinner.fail(pc.red(`Failed to process ${mapping.platform}`));
      throw error;
    }
  }

  return version_obj;
}

program
  .name("release-manager")
  .description("Manage Iron Borders releases")
  .version("1.0.0");

program
  .command("download [version] [run-id]")
  .description("Download and extract GitHub Actions artifacts")
  .action(async (version, runId) => {
    try {
      const [owner, repo] = process.env.GITHUB_REPOSITORY!.split('/');
      const token = process.env.GITHUB_TOKEN!;
      const octokit = new Octokit({ auth: token });

      if (!version && !runId) {
        await listRecentRuns(octokit, owner, repo);
        throw new Error(
          "No version and run ID specified. Provide both: download <version> <run-id>"
        );
      }

      if (!version || !runId) {
        throw new Error(
          "Both version and run ID are required: download <version> <run-id>"
        );
      }

      const runIdNum = parseInt(runId, 10);

      const run = await octokit.rest.actions.getWorkflowRun({
        owner,
        repo,
        run_id: runIdNum,
      });

      if (run.data.status !== "completed") {
        throw new Error(
          `Cannot download artifacts from run ${runId}: workflow is still ${run.data.status}`
        );
      }

      console.log(
        pc.bold(pc.cyan(`\n📦 Downloading artifacts from run ${runId}\n`))
      );

      const artifacts = await octokit.rest.actions.listWorkflowRunArtifacts({
        owner,
        repo,
        run_id: runIdNum,
      });

      const releasesDir = getReleasesDir();
      const versionDir = resolve(releasesDir, version);
      await mkdir(versionDir, { recursive: true });

      for (const artifactName of artifactNames) {
        const artifact = artifacts.data.artifacts.find(
          (a) => a.name === artifactName
        );

        if (!artifact) {
          console.log(
            pc.yellow(`⚠️  Artifact ${artifactName} not found, skipping`)
          );
          continue;
        }

        const spinner = ora(`Downloading ${artifactName}`).start();

        try {
          const zipPath = resolve(versionDir, `${artifactName}.zip`);
          await downloadArtifact(octokit, owner, repo, artifact.id, zipPath);

          spinner.text = `Extracting ${artifactName}`;
          const extractDir = resolve(versionDir, artifactName);
          await extractZip(zipPath, extractDir);

          await unlink(zipPath);

          spinner.succeed(pc.green(`Downloaded and extracted ${artifactName}`));
        } catch (error) {
          spinner.fail(pc.red(`Failed to download ${artifactName}`));
          throw error;
        }
      }

      console.log(
        pc.bold(pc.green(`\n✅ All artifacts downloaded to ${versionDir}\n`))
      );
    } catch (error) {
      console.error(
        pc.red("\n❌ Error:"),
        error instanceof Error ? error.message : error
      );
      process.exit(1);
    }
  });

program
  .command("upload <version>")
  .description("Upload release artifacts to R2 and update manifest")
  .option("--latest", "Set this version as latest")
  .option("--skip-upload", "Skip upload, only generate manifest locally")
  .action(async (version, options) => {
    try {
      const { latest, skipUpload } = options;
      const bucket = process.env.R2_BUCKET_NAME!;
      const baseUrl = process.env.R2_BASE_URL!;

      console.log(pc.bold(pc.cyan(`\n🚀 Uploading release ${version}\n`)));

      const s3Client = createR2Client();

      const spinner = ora("Loading existing manifest").start();
      let manifest: Manifest;
      try {
        manifest = await loadManifest(bucket, s3Client);
        spinner.succeed("Loaded manifest");
      } catch (error: any) {
        if (error.message.includes("not found")) {
          spinner.warn("Manifest not found, creating new one");
          manifest = await createManifest();
        } else {
          spinner.fail("Failed to load manifest");
          throw error;
        }
      }

      const versionObj = await processVersion(
        version,
        bucket,
        baseUrl,
        skipUpload,
        s3Client
      );

      const existingIndex = manifest.versions.findIndex(
        (v) => v.version === version
      );
      if (existingIndex >= 0) {
        manifest.versions[existingIndex] = versionObj;
        console.log(
          pc.yellow(`\n⚠️  Updated existing version ${version} in manifest`)
        );
      } else {
        manifest.versions.unshift(versionObj);
        console.log(pc.green(`\n✅ Added new version ${version} to manifest`));
      }

      manifest.versions.sort((a, b) =>
        b.version.localeCompare(a.version, undefined, { numeric: true })
      );

      const shouldSetLatest =
        latest || !manifest.latest || manifest.latest < version;
      if (shouldSetLatest) {
        manifest.latest = version;
        console.log(pc.cyan(`📌 Set ${version} as latest version`));
      }

      if (!skipUpload) {
        const manifestSpinner = ora("Uploading manifest").start();
        await saveManifest(manifest, bucket, s3Client);
        manifestSpinner.succeed("Uploaded manifest");
      } else {
        console.log(pc.yellow("\n⚠️  Skipped upload (--skip-upload flag)"));
        console.log("\nGenerated manifest:");
        console.log(JSON.stringify(manifest, null, 2));
      }

      console.log(
        pc.bold(pc.green(`\n✅ Release ${version} uploaded successfully!\n`))
      );
    } catch (error) {
      console.error(pc.red("\n❌ Error:"), error);
      process.exit(1);
    }
  });

program
  .command("list")
  .description("List all available releases")
  .action(async () => {
    try {
      const bucket = process.env.R2_BUCKET_NAME!;

      const s3Client = createR2Client();
      const manifest = await loadManifest(bucket, s3Client);

      console.log(pc.bold(pc.cyan("\n📦 Available Releases\n")));
      console.log(
        pc.dim(
          `${"Version".padEnd(12)} ${"Visible".padEnd(10)} ${"Released".padEnd(
            30
          )} ${"Status"}`
        )
      );
      console.log(pc.dim("─".repeat(80)));

      for (const version of manifest.versions) {
        const isLatest = version.version === manifest.latest;
        const versionStr = isLatest
          ? pc.green(version.version)
          : version.version;
        const visibleStr = version.visible ? pc.green("✓ Yes") : pc.red("✗ No");
        const releasedStr = new Date(version.released).toLocaleString();
        const statusStr = isLatest ? pc.cyan(" (latest)") : "";

        console.log(
          `${padWithColor(versionStr, 12)} ${padWithColor(
            visibleStr,
            10
          )} ${releasedStr.padEnd(30)} ${statusStr}`
        );
      }

      console.log("");
    } catch (error: any) {
      if (error.message.includes("not found")) {
        console.error(
          pc.red("\n❌ No releases found (manifest.json does not exist)\n")
        );
        process.exit(1);
      }

      console.error(pc.red("\n❌ Error:"), error.message);
      process.exit(1);
    }
  });

program
  .command("set-latest")
  .description("Set a version as the latest in the manifest")
  .requiredOption(
    "-v, --version <version>",
    "Version to set as latest (e.g., 0.5.7)"
  )
  .action(async (options) => {
    try {
      const { version } = options;
      const bucket = process.env.R2_BUCKET_NAME!;

      const s3Client = createR2Client();

      const spinner = ora("Loading manifest").start();
      const manifest = await loadManifest(bucket, s3Client);
      spinner.succeed("Loaded manifest");

      const versionExists = manifest.versions.some(
        (v) => v.version === version
      );

      if (!versionExists) {
        console.error(pc.red(`\n❌ Version ${version} not found in manifest`));
        console.log(pc.yellow("\nAvailable versions:"));
        manifest.versions.forEach((v) => {
          const indicator =
            v.version === manifest.latest ? pc.green(" (current latest)") : "";
          console.log(`  - ${v.version}${indicator}`);
        });
        process.exit(1);
      }

      const previousLatest = manifest.latest;
      manifest.latest = version;

      const uploadSpinner = ora("Updating manifest").start();
      await saveManifest(manifest, bucket, s3Client);
      uploadSpinner.succeed("Updated manifest");

      console.log(pc.bold(pc.green(`\n✅ Latest version updated:`)));
      console.log(`   ${previousLatest} → ${pc.cyan(version)}\n`);
    } catch (error) {
      console.error(pc.red("\n❌ Error:"), error);
      process.exit(1);
    }
  });

program.parse();
