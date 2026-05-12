export type ViewMode = 'dashboard' | 'map' | 'link-analysis' | 'intelligence' | 'agent' | 'vault' | 'settings';

export const activeView = $state({
  current: 'dashboard' as ViewMode
});

export const globalSearchOpen = $state({
  isOpen: false
});
