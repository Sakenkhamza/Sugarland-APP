import fs from 'fs';
import path from 'path';
import https from 'https';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
async function main() {
    const token = process.argv[2];
    const repo = process.argv[3];
    const tagName = process.argv[4];

    if (!token || !repo || !tagName) {
        console.error("Usage: node generate-updater.js <token> <repo> <tag>");
        process.exit(1);
    }

    // Wait for the release assets to be uploaded by the previous step
    console.log('Waiting for assets to be uploaded by Tauri action...');
    await new Promise(r => setTimeout(r, 15000));

    console.log(`Fetching release info for ${repo} tag ${tagName}`);

    const options = {
        hostname: 'api.github.com',
        path: `/repos/${repo}/releases/tags/${tagName}`,
        headers: {
            'User-Agent': 'Node.js',
            'Authorization': `token ${token}`
        }
    };

    const release = await new Promise((resolve, reject) => {
        https.get(options, (res) => {
            let data = '';
            res.on('data', chunk => data += chunk);
            res.on('end', () => {
                if (res.statusCode === 200) {
                    resolve(JSON.parse(data));
                } else {
                    reject(new Error(`Failed to fetch release: ${res.statusCode} ${data}`));
                }
            });
        }).on('error', reject);
    });

    const assets = release.assets;
    if (!assets || assets.length === 0) {
        console.error("No assets found for this release.");
        process.exit(1);
    }

    const updater = {
        version: tagName.replace(/^v/, ''), // strip 'v' prefix
        notes: release.body || "A new version of Sugarland is available!",
        pub_date: release.published_at,
        platforms: {}
    };

    let foundAssets = 0;

    for (const asset of assets) {
        const name = asset.name;
        const url = asset.browser_download_url;

        // Detect platforms
        const isWindowsExe = name.endsWith('-setup.exe.sig');
        const isWindowsMsi = name.endsWith('.msi.sig');
        const isMacIntel = name.endsWith('x86_64.app.tar.gz.sig');
        const isMacArm = name.endsWith('aarch64.app.tar.gz.sig') || name.endsWith('arm64.app.tar.gz.sig');
        const isMacUniversal = name.endsWith('universal.app.tar.gz.sig');

        if (!isWindowsExe && !isWindowsMsi && !isMacIntel && !isMacArm && !isMacUniversal) continue;

        // Fetch the signature content
        const sigUrl = url;
        const sigOptions = {
            headers: {
                'User-Agent': 'Node.js',
            }
        };

        console.log(`Fetching signature for ${name} at ${sigUrl}`);
        const sigContent = await new Promise((resolve, reject) => {
            https.get(sigUrl, sigOptions, (res) => {
                if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
                    // Handle redirect
                    https.get(res.headers.location, sigOptions, (res2) => {
                        let sData = '';
                        res2.on('data', chunk => sData += chunk);
                        res2.on('end', () => resolve(sData.trim()));
                    }).on('error', reject);
                } else {
                    let sData = '';
                    res.on('data', chunk => sData += chunk);
                    res.on('end', () => resolve(sData.trim()));
                }
            }).on('error', reject);
        });

        // Determine the original asset URL (without .sig)
        const originalAssetUrl = url.replace(/\.sig$/, '');

        const platformData = {
            signature: sigContent,
            url: originalAssetUrl
        };

        // For Windows, default to exe (NSIS) over MSI to avoid overriding unless needed.
        if (isWindowsExe) {
            updater.platforms['windows-x86_64'] = platformData;
        } else if (isWindowsMsi && !updater.platforms['windows-x86_64']) {
            updater.platforms['windows-x86_64'] = platformData;
        } else if (isMacIntel) {
            updater.platforms['darwin-x86_64'] = platformData;
        } else if (isMacArm) {
            updater.platforms['darwin-aarch64'] = platformData;
        } else if (isMacUniversal) {
            // Provide for both if universal
            updater.platforms['darwin-x86_64'] = platformData;
            updater.platforms['darwin-aarch64'] = platformData;
        }
        foundAssets++;
    }

    if (foundAssets === 0) {
        console.error("No .sig files found. Make sure TAURI_SIGNING_PRIVATE_KEY is set in secrets.");
        process.exit(1);
    }

    const outDir = path.join(__dirname, '..', 'gh-pages-dist');
    if (!fs.existsSync(outDir)) {
        fs.mkdirSync(outDir, { recursive: true });
    }

    const outFile = path.join(outDir, 'updater.json');
    fs.writeFileSync(outFile, JSON.stringify(updater, null, 2));

    console.log(`Created updater.json at ${outFile}`);
    console.log(JSON.stringify(updater, null, 2));
}

main().catch(err => {
    console.error(err);
    process.exit(1);
});
