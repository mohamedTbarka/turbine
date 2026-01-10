<script lang="ts">
	import { onMount } from 'svelte';
	import { apiClient, type WorkerInfo } from '$lib/api/client';
	import { formatDistanceToNow } from 'date-fns';

	let workers: WorkerInfo[] = [];
	let loading = true;
	let error: string | null = null;

	async function loadWorkers() {
		try {
			loading = true;
			error = null;
			const response = await apiClient.getWorkers();
			workers = response.workers;
		} catch (e: any) {
			error = e.message;
			console.error('Failed to load workers:', e);
		} finally {
			loading = false;
		}
	}

	onMount(() => {
		loadWorkers();

		const interval = setInterval(loadWorkers, 5000);
		return () => clearInterval(interval);
	});

	function formatUptime(startedAt: string): string {
		try {
			return formatDistanceToNow(new Date(startedAt), { addSuffix: false });
		} catch {
			return '-';
		}
	}

	function getStatusColor(status: string): string {
		switch (status) {
			case 'active':
				return 'bg-green-100 text-green-800';
			case 'idle':
				return 'bg-gray-100 text-gray-800';
			default:
				return 'bg-red-100 text-red-800';
		}
	}
</script>

<svelte:head>
	<title>Workers - Turbine Dashboard</title>
</svelte:head>

<div class="p-8">
	<div class="mb-8">
		<h1 class="text-3xl font-bold text-gray-900">Workers</h1>
		<p class="text-gray-600 mt-2">Monitor active workers and their status</p>
	</div>

	{#if loading && workers.length === 0}
		<div class="flex items-center justify-center h-64">
			<div class="animate-spin rounded-full h-12 w-12 border-b-2 border-primary-600"></div>
		</div>
	{:else if error}
		<div class="bg-red-50 border border-red-200 rounded-lg p-4 text-red-800">
			Error: {error}
		</div>
	{:else}
		<!-- Workers Grid -->
		<div class="grid grid-cols-1 lg:grid-cols-2 xl:grid-cols-3 gap-6">
			{#each workers as worker}
				<div class="card hover:shadow-lg transition-shadow">
					<!-- Header -->
					<div class="flex items-start justify-between mb-4">
						<div class="flex-1 min-w-0">
							<div class="flex items-center space-x-2">
								<h3 class="text-sm font-mono font-semibold text-gray-900 truncate">
									{worker.id.substring(0, 12)}...
								</h3>
								<span class="badge {getStatusColor(worker.status)}">
									{worker.status}
								</span>
							</div>
							<p class="text-xs text-gray-600 mt-1">{worker.hostname}</p>
						</div>
					</div>

					<!-- Stats -->
					<div class="grid grid-cols-2 gap-3 mb-4">
						<div>
							<div class="text-xs text-gray-500">Tasks Processed</div>
							<div class="text-xl font-bold text-primary-600">{worker.tasks_processed}</div>
						</div>
						<div>
							<div class="text-xs text-gray-500">Concurrency</div>
							<div class="text-xl font-bold text-purple-600">{worker.concurrency}</div>
						</div>
						<div class="col-span-2">
							<div class="text-xs text-gray-500">Queues</div>
							<div class="flex flex-wrap gap-1 mt-1">
								{#each worker.queues as queue}
									<span class="badge-info text-xs">{queue}</span>
								{/each}
							</div>
						</div>
					</div>

					<!-- Current Task -->
					{#if worker.current_task}
						<div class="bg-yellow-50 border border-yellow-200 rounded p-2 mb-3">
							<div class="text-xs text-yellow-800">Currently Processing:</div>
							<div class="text-sm font-mono text-yellow-900 truncate mt-1">
								{worker.current_task}
							</div>
						</div>
					{:else}
						<div class="bg-gray-50 border border-gray-200 rounded p-2 mb-3">
							<div class="text-xs text-gray-600 text-center">Idle</div>
						</div>
					{/if}

					<!-- Footer -->
					<div class="text-xs text-gray-500 flex justify-between pt-3 border-t border-gray-200">
						<span>PID: {worker.pid}</span>
						<span>Uptime: {formatUptime(worker.started_at)}</span>
					</div>
				</div>
			{/each}
		</div>

		{#if workers.length === 0}
			<div class="card text-center py-12 text-gray-500">
				No active workers found
			</div>
		{/if}
	{/if}
</div>
