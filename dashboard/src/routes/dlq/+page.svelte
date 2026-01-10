<script lang="ts">
	import { onMount } from 'svelte';
	import { apiClient, type DLQTask } from '$lib/api/client';
	import { formatDistanceToNow } from 'date-fns';

	let failedTasks: DLQTask[] = [];
	let loading = true;
	let error: string | null = null;
	let selectedQueue = 'default';
	let limit = 50;
	let offset = 0;
	let total = 0;

	let selectedTask: DLQTask | null = null;
	let showModal = false;

	async function loadDLQ() {
		try {
			loading = true;
			error = null;
			const response = await apiClient.getDLQ(selectedQueue, { limit, offset });
			failedTasks = response.tasks || [];
			total = response.total_failed || 0;
		} catch (e: any) {
			error = e.message;
			console.error('Failed to load DLQ:', e);
		} finally {
			loading = false;
		}
	}

	async function reprocessAll() {
		if (!confirm('Reprocess all failed tasks in DLQ?')) {
			return;
		}

		try {
			await apiClient.reprocessDLQ(selectedQueue);
			alert('Tasks requeued for processing');
			await loadDLQ();
		} catch (e: any) {
			alert(`Failed to reprocess: ${e.message}`);
		}
	}

	async function purgeDLQ() {
		if (!confirm('Permanently delete all failed tasks from DLQ?')) {
			return;
		}

		try {
			const result = await apiClient.purgeDLQ(selectedQueue);
			alert(`Purged ${result.purged} tasks from DLQ`);
			await loadDLQ();
		} catch (e: any) {
			alert(`Failed to purge DLQ: ${e.message}`);
		}
	}

	function viewTask(task: DLQTask) {
		selectedTask = task;
		showModal = true;
	}

	onMount(() => {
		loadDLQ();
	});
</script>

<svelte:head>
	<title>Dead Letter Queue - Turbine Dashboard</title>
</svelte:head>

<div class="p-8">
	<div class="mb-8 flex items-center justify-between">
		<div>
			<h1 class="text-3xl font-bold text-gray-900">Dead Letter Queue</h1>
			<p class="text-gray-600 mt-2">Failed tasks after max retries</p>
		</div>

		<div class="flex space-x-3">
			<button on:click={reprocessAll} class="btn-primary"> Reprocess All </button>
			<button on:click={purgeDLQ} class="btn-danger"> Purge DLQ </button>
		</div>
	</div>

	<!-- Stats -->
	<div class="card mb-6 bg-red-50 border-red-200">
		<div class="flex items-center justify-between">
			<div>
				<div class="text-sm text-red-700">Failed Tasks in DLQ</div>
				<div class="text-3xl font-bold text-red-900">{total}</div>
			</div>
			<div class="text-6xl">⚠️</div>
		</div>
	</div>

	{#if loading}
		<div class="flex items-center justify-center h-64">
			<div class="animate-spin rounded-full h-12 w-12 border-b-2 border-primary-600"></div>
		</div>
	{:else if error}
		<div class="bg-red-50 border border-red-200 rounded-lg p-4 text-red-800">
			Error: {error}
		</div>
	{:else}
		<!-- Failed Tasks List -->
		<div class="space-y-4">
			{#each failedTasks as task}
				<div class="card hover:shadow-lg transition-shadow cursor-pointer" on:click={() => viewTask(task)}>
					<div class="flex items-start justify-between">
						<div class="flex-1">
							<div class="flex items-center space-x-3 mb-2">
								<h3 class="font-semibold text-gray-900">{task.task_name}</h3>
								<span class="badge-error">Failed</span>
								<span class="text-xs text-gray-500">{task.retries} retries</span>
							</div>

							<div class="text-sm font-mono text-gray-600 mb-2">{task.task_id}</div>

							<div class="bg-red-50 border border-red-200 rounded p-2 mb-2">
								<div class="text-xs text-red-700 font-medium">Error:</div>
								<div class="text-sm text-red-800 mt-1 line-clamp-2">{task.error}</div>
							</div>

							<div class="flex items-center space-x-4 text-xs text-gray-500">
								<span>Failed: {formatDistanceToNow(new Date(task.failed_at), { addSuffix: true })}</span>
								<span>Worker: {task.worker_id?.substring(0, 8) || 'unknown'}</span>
							</div>
						</div>

						<button
							on:click|stopPropagation={() => viewTask(task)}
							class="text-primary-600 hover:text-primary-800 text-sm font-medium"
						>
							View Details
						</button>
					</div>
				</div>
			{/each}
		</div>

		{#if failedTasks.length === 0}
			<div class="card text-center py-12">
				<div class="text-6xl mb-4">✅</div>
				<div class="text-xl font-semibold text-gray-900">DLQ is empty!</div>
				<div class="text-gray-600 mt-2">No failed tasks found.</div>
			</div>
		{/if}
	{/if}
</div>

<!-- Task Detail Modal -->
{#if showModal && selectedTask}
	<div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4" on:click={() => (showModal = false)}>
		<div class="bg-white rounded-lg max-w-4xl w-full max-h-[90vh] overflow-auto" on:click|stopPropagation>
			<div class="sticky top-0 bg-white border-b px-6 py-4 flex justify-between items-center">
				<h2 class="text-xl font-bold">Failed Task Details</h2>
				<button on:click={() => (showModal = false)} class="text-gray-400 hover:text-gray-600 text-2xl">
					&times;
				</button>
			</div>

			<div class="p-6 space-y-4">
				<div>
					<div class="text-xs text-gray-500">Task ID</div>
					<div class="font-mono text-sm mt-1">{selectedTask.task_id}</div>
				</div>

				<div>
					<div class="text-xs text-gray-500">Task Name</div>
					<div class="text-sm mt-1">{selectedTask.task_name}</div>
				</div>

				<div>
					<div class="text-xs text-gray-500">Error</div>
					<div class="bg-red-50 border border-red-200 rounded p-3 mt-1">
						<pre class="text-sm text-red-800 whitespace-pre-wrap">{selectedTask.error}</pre>
					</div>
				</div>

				{#if selectedTask.traceback}
					<div>
						<div class="text-xs text-gray-500">Traceback</div>
						<div class="bg-gray-50 border rounded p-3 mt-1 overflow-auto max-h-64">
							<pre class="text-xs font-mono">{selectedTask.traceback}</pre>
						</div>
					</div>
				{/if}

				<div>
					<div class="text-xs text-gray-500">Arguments</div>
					<div class="bg-gray-50 border rounded p-3 mt-1">
						<pre class="text-xs">{JSON.stringify({ args: selectedTask.args, kwargs: selectedTask.kwargs }, null, 2)}</pre>
					</div>
				</div>
			</div>

			<div class="sticky bottom-0 bg-gray-50 px-6 py-4 flex justify-end space-x-3 border-t">
				<button on:click={() => (showModal = false)} class="btn-secondary"> Close </button>
			</div>
		</div>
	</div>
{/if}
