export type ToastType = 'success' | 'error' | 'info' | 'loading';

export interface Toast {
	id: string;
	type: ToastType;
	message: string;
	duration?: number;
}

class ToastStore {
	toasts = $state<Toast[]>([]);

	add(toast: Omit<Toast, 'id'>): string {
		const id = Math.random().toString(36).substring(2, 9);
		this.toasts.push({ ...toast, id });

		if (toast.type !== 'loading' && toast.duration !== 0) {
			setTimeout(() => this.remove(id), toast.duration || 4000);
		}
		return id;
	}

	updateOne(id: string, updates: Partial<Toast>) {
		const toast = this.toasts.find((t) => t.id === id);
		if (toast) Object.assign(toast, updates);

		if (updates.type && updates.type !== 'loading') {
			const duration = updates.duration || 4000;
			if (duration !== 0) {
				setTimeout(() => this.remove(id), duration);
			}
		}
	}

	remove(id: string) {
		this.toasts = this.toasts.filter((t) => t.id !== id);
	}
}

export const toastStore = new ToastStore();

// Free-function API kept for the 8 existing call sites that only ever needed to fire a toast,
// not touch the store directly — avoids a mechanical import-and-rename pass across files that
// have nothing to do with this migration.
export const addToast = (toast: Omit<Toast, 'id'>) => toastStore.add(toast);
export const updateToast = (id: string, updates: Partial<Toast>) => toastStore.updateOne(id, updates);
export const removeToast = (id: string) => toastStore.remove(id);
