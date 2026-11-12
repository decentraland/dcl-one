/** Example states for ch-capture.mjs: the signed-out Home page and the update check in App Settings. */

const CREATE = '[data-testid="create-button"]';

const openUpdateCheck = async ({ page }) => {
  await page.click('[data-testid="logo-menu-button"]');
  await page.click('[data-testid="logo-menu-check-updates"]');
};

export default {
  viewport: { width: 1440, height: 900 },
  states: [
    { name: 'home', component: 'home', ready: '.HomeRow' },
    { name: 'home-es', component: 'home', locale: 'es', ready: '.HomeRow' },
    {
      name: 'update-up-to-date',
      component: 'update-check',
      steps: openUpdateCheck,
      ready: '.update-settings__up-to-date-message',
    },
    {
      name: 'update-checking',
      component: 'update-check',
      ipc: {
        'updater.checkForUpdates': (_event, config) =>
          config?.autoDownload
            ? { success: true, value: { updateAvailable: false, version: '0.0.0' } }
            : new Promise(() => {}),
      },
      steps: openUpdateCheck,
      ready: '.update-settings__button-area button:disabled',
    },
    {
      name: 'update-available',
      component: 'update-check',
      ipc: {
        'updater.checkForUpdates': {
          success: true,
          value: { updateAvailable: true, version: '9.9.9' },
        },
        'updater.getReleaseNotes': {
          success: true,
          value: {
            version: '9.9.9',
            content: '## Bug Fixes\n\n- Example fix for the release notes.',
          },
        },
      },
      steps: openUpdateCheck,
      ready: '.update-settings__release-notes-header',
    },
    { name: 'create-rest', component: 'create-button', ready: CREATE },
    {
      name: 'create-hover',
      component: 'create-button',
      steps: ({ page }) => page.hover(CREATE),
      ready: CREATE,
    },
  ],
};
