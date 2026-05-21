export type ViewMode =
	| 'dashboard'
	| 'map'
	| 'link-analysis'
	| 'intelligence'
	| 'agent'
	| 'vault'
	| 'settings';

class AppStore {
	activeView = $state<ViewMode>('dashboard');
	globalSearchOpen = $state(false);
	selectedRecordId = $state<string | null>(null);

	setRecord(id: string | null) {
		this.selectedRecordId = id;
	}

	setView(view: ViewMode) {
		this.activeView = view;
	}
}

export const appStore = new AppStore();
