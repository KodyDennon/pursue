/**
 * Maps a record's analysis_status to the shared semantic status class used to drive badge
 * color/styling. The exact label text shown next to this class varies per view (GridView,
 * IntelCardsView, etc. each have their own copy) — this only centralizes the status ->
 * semantic-class mapping, which was previously the same 5-way branch copy-pasted verbatim in
 * three separate places.
 */
export type RecordStatusClass = 'ready' | 'indexed' | 'busy' | 'pending' | 'error' | 'unknown';

export function getRecordStatusClass(status: string | null | undefined): RecordStatusClass {
	switch (status) {
		case 'completed':
			return 'ready';
		case 'indexed':
			return 'indexed';
		case 'synthesizing':
			return 'busy';
		case 'indexing':
		case 'extracting-foundation':
			return 'pending';
		case 'failed':
			return 'error';
		default:
			return 'unknown';
	}
}
