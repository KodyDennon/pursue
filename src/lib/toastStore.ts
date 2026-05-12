import { writable } from 'svelte/store';

export type ToastType = 'success' | 'error' | 'info' | 'loading';

export interface Toast {
	id: string;
	type: ToastType;
	message: string;
	duration?: number;
}

export const toasts = writable<Toast[]>([]);

export const addToast = (toast: Omit<Toast, 'id'>) => {
	const id = Math.random().toString(36).substring(2, 9);
	const newToast = { ...toast, id };
	toasts.update((all) => [...all, newToast]);

	if (toast.type !== 'loading' && toast.duration !== 0) {
		setTimeout(() => removeToast(id), toast.duration || 4000);
	}
	return id;
};

export const updateToast = (id: string, updates: Partial<Toast>) => {
	toasts.update((all) => all.map((t) => (t.id === id ? { ...t, ...updates } : t)));

	if (updates.type && updates.type !== 'loading') {
		const duration = updates.duration || 4000;
		if (duration !== 0) {
			setTimeout(() => removeToast(id), duration);
		}
	}
};

export const removeToast = (id: string) => {
	toasts.update((all) => all.filter((t) => t.id !== id));
};
