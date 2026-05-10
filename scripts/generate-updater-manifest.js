import fs from 'fs';
import path from 'path';
import { execSync } from 'child_process';

const tagName = process.env.TAG_NAME;
if (!tagName) {
  console.error('TAG_NAME env var is required');
  process.exit(1);
}

const version = tagName.startsWith('v') ? tagName.slice(1) : tagName;
const releaseNotes = `System Intelligence Release ${tagName}`;
const pubDate = new Date().toISOString();

async function generate() {
  console.log(`Generating manifest for version ${version}...`);

  let assets = [];
  let attempts = 0;
  const maxAttempts = 5;

  while (attempts < maxAttempts) {
    attempts++;
    console.log(`Attempt ${attempts}: Fetching release assets for ${tagName}...`);
    const assetsJson = execSync(`gh release view ${tagName} --json assets`).toString();
    const data = JSON.parse(assetsJson);
    assets = data.assets || [];
    
    console.log(`Debug: Found ${assets.length} total assets.`);
    assets.forEach(a => console.log(`  - ${a.name}`));

    // Check if we have both installers and signatures
    const hasSigs = assets.some(a => a.name.endsWith('.sig'));
    const hasInstallers = assets.some(a => a.name.endsWith('.tar.gz') || a.name.endsWith('.msi') || a.name.endsWith('.AppImage'));

    if (hasSigs && hasInstallers) {
      console.log('Found both installers and signatures.');
      break;
    }

    if (attempts < maxAttempts) {
      console.log('Missing signatures or installers. Waiting 30 seconds...');
      await new Promise(resolve => setTimeout(resolve, 30000));
    }
  }

  const manifest = {
    version,
    notes: releaseNotes,
    pub_date: pubDate,
    platforms: {}
  };

  const downloadAsset = (name) => {
    console.log(`Downloading ${name}...`);
    execSync(`gh release download ${tagName} -p "${name}" --clobber`);
  };

  for (const asset of assets) {
    const { name, url } = asset;
    
    // Tauri v2 updater looks for .sig files
    if (name.endsWith('.sig')) {
      const targetAssetName = name.replace('.sig', '');
      const targetAsset = assets.find(a => a.name === targetAssetName);
      
      if (targetAsset) {
        downloadAsset(name);
        const signature = fs.readFileSync(name, 'utf8').trim();
        
        // Determine platform from filename
        // Examples: 
        // PURSUE.Data.Analyzer_aarch64.app.tar.gz.sig -> darwin-aarch64
        // PURSUE.Data.Analyzer_x64.msi.zip.sig -> windows-x86_64
        
        let platforms = [];
        if (name.includes('aarch64.app.tar.gz')) {
          platforms = ['darwin-aarch64', 'darwin-aarch64-app'];
        } else if (name.includes('x64.app.tar.gz')) {
          platforms = ['darwin-x86_64', 'darwin-x86_64-app'];
        } else if (name.includes('x64') && (name.includes('.msi.zip') || name.includes('.nsis.zip') || name.includes('.zip'))) {
          platforms = ['windows-x86_64'];
        } else if (name.includes('amd64.deb') || name.includes('x86_64.AppImage') || name.includes('x86_64.tar.gz')) {
          platforms = ['linux-x86_64'];
        } else if (name.includes('aarch64.deb') || name.includes('aarch64.AppImage') || name.includes('aarch64.tar.gz')) {
          platforms = ['linux-aarch64'];
        }

        for (const platform of platforms) {
          console.log(`Found signature for ${platform}: ${name}`);
          manifest.platforms[platform] = {
            signature,
            url: targetAsset.url
          };
        }
      } else {
        console.warn(`Could not find target asset for signature: ${name}`);
      }
    }
  }

  if (Object.keys(manifest.platforms).length === 0) {
    console.error('No platforms found in release assets!');
    process.exit(1);
  }

  fs.writeFileSync('latest.json', JSON.stringify(manifest, null, 2));
  console.log('Manifest generated successfully:');
  console.log(JSON.stringify(manifest, null, 2));
}

generate().catch(err => {
  console.error(err);
  process.exit(1);
});
