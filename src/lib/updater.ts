import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { addToast } from './toastStore';

/**
 * Checks for system updates and handles the download/install process.
 * @param silent If true, only notifies on failure or success in manual mode.
 */
export async function checkForUpdates(silent = false) {
  try {
    const update = await check();
    if (update) {
      console.log(`Update found: ${update.version} (${update.date})`);
      
      addToast({
        type: 'info',
        message: `Intelligence Update Available: v${update.version}. Initiating secure download...`,
        duration: 5000
      });

      let downloaded = 0;
      let contentLength = 0;

      await update.downloadAndInstall((event) => {
        switch (event.event) {
          case 'Started':
            contentLength = event.data.contentLength ?? 0;
            console.log(`Started downloading ${contentLength} bytes`);
            break;
          case 'Progress':
            downloaded += event.data.chunkLength;
            // Progress could be hooked into a UI bar if needed
            break;
          case 'Finished':
            console.log('Download finished');
            break;
        }
      });

      addToast({
        type: 'success',
        message: 'Intelligence core patched. Relaunching system...',
        duration: 3000
      });

      // Brief delay for user to read toast
      setTimeout(async () => {
        await relaunch();
      }, 2000);

    } else if (!silent) {
      addToast({
        type: 'success',
        message: 'System intelligence is at the latest version.',
        duration: 2000
      });
    }
  } catch (error) {
    console.error('Update Check Error:', error);
    if (!silent) {
      addToast({
        type: 'error',
        message: `Update verification failed: ${error}`,
      });
    }
  }
}
