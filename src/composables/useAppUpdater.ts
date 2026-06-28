import { check } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'
import { useConfirm } from 'primevue/useconfirm'
import { useToast } from 'primevue/usetoast'

export function useAppUpdater() {
  const confirm = useConfirm()
  const toast = useToast()

  async function checkForUpdate() {
    try {
      const update = await check()
      if (!update?.available) return

      confirm.require({
        header: `Update Available`,
        message: `Suvarix v${update.version} is ready to install. Install now and restart?`,
        icon: 'pi pi-download',
        acceptLabel: 'Install & Restart',
        rejectLabel: 'Later',
        accept: async () => {
          toast.add({
            severity: 'info',
            summary: 'Downloading update…',
            detail: `Suvarix v${update.version}`,
            life: 0,
            closable: false,
          })
          try {
            await update.downloadAndInstall()
            await relaunch()
          } catch (e) {
            toast.add({ severity: 'error', summary: 'Update failed', detail: String(e), life: 5000 })
          }
        },
      })
    } catch (e) {
      console.error('[updater] check failed:', e)
    }
  }

  return { checkForUpdate }
}
