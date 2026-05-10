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

  // Use GH CLI to list assets
  const assetsJson = execSync(`gh release view ${tagName} --json assets`).toString();
  const { assets } = JSON.parse(assetsJson);

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
        // PURSUE_0.2.2_aarch64.app.tar.gz.sig -> darwin-aarch64
        // PURSUE_0.2.2_x64_en-US.msi.zip.sig -> windows-x86_64
        // PURSUE_0.2.2_amd64.deb.sig -> linux-x86_64
        
        let platform = null;
        if (name.includes('aarch64.app.tar.gz')) platform = 'darwin-aarch64';
        else if (name.includes('x64.app.tar.gz')) platform = 'darwin-x86_64';
        else if (name.includes('x64') && name.includes('.msi')) platform = 'windows-x86_64';
        else if (name.includes('amd64.deb') || name.includes('x86_64.AppImage')) platform = 'linux-x86_64';
        else if (name.includes('aarch64.deb')) platform = 'linux-aarch64';

        if (platform) {
          console.log(`Found signature for ${platform}: ${name}`);
          manifest.platforms[platform] = {
            signature,
            url: targetAsset.url
          };
        }
      }
    }
  }

  fs.writeFileSync('latest.json', JSON.stringify(manifest, null, 2));
  console.log('Manifest generated successfully:');
  console.log(JSON.stringify(manifest, null, 2));
}

generate().catch(err => {
  console.error(err);
  process.exit(1);
});
