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

	initializing = $state(true);
	bootLogs = $state<string[]>([]);

	addBootLog(msg: string) {
		this.bootLogs.push(`[${new Date().toLocaleTimeString()}] ${msg}`);
		if (this.bootLogs.length > 5) this.bootLogs.shift();
	}

	setRecord(id: string | null) {
		this.selectedRecordId = id;
	}

	setView(view: ViewMode) {
		this.activeView = view;
	}
}

export const appStore = new AppStore();
