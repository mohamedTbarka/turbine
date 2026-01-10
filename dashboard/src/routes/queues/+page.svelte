<script lang="ts">
	import { onMount } from 'svelte';
	import { apiClient, type QueueInfo } from '$lib/api/client';

	let queues: QueueInfo[] = [];
	let loading = true;
	let error: string | null = null;

	async function loadQueues() {
		try {
			loading = true;
			error = null;
			const response = await apiClient.getQueues();
			queues = response.queues;
		} catch (e: any) {
			error = e.message;
			console.error('Failed to load queues:', e);
		} finally {
			loading = false;
		}
	}

	async function purgeQueue(queueName: string) {
		if (!confirm(`Are you sure you want to purge all tasks from queue "${queueName}"?`)) {
			return;
		}

		try {
			const result = await apiClient.purgeQueue(queueName);
			alert(`Purged ${result.purged} tasks from ${queueName}`);
			await loadQueues();
		} catch (e: any) {
			alert(`Failed to purge queue: ${e.message}`);
		}
	}

	onMount(() => {
		loadQueues();

		const interval = setInterval(loadQueues, 5000);
		return () => clearInterval(interval);
	});

	$: totalPending = queues.reduce((sum, q) => sum + q.pending, 0);
	$: totalProcessing = queues.reduce((sum, q) => sum + q.processing, 0);
</script>

<svelte:head>
	<title>Queues - Turbine Dashboard</title>
</svelte:head>

<div class="p-8">
	<div class="mb-8">
		<h1 class="text-3xl font-bold text-gray-900">Queues</h1>
		<p class="text-gray-600 mt-2">Monitor queue depths and throughput</p>
	</div>

	<!-- Summary Cards -->
	<div class="grid grid-cols-1 md:grid-cols-3 gap-6 mb-8">
		<div class="card">
			<div class="text-sm text-gray-600 mb-2">Total Queues</div>
			<div class="text-3xl font-bold text-primary-600">{queues.length}</div>
		</div>

		<div class="card">
			<div class="text-sm text-gray-600 mb-2">Pending Tasks</div>
			<div class="text-3xl font-bold text-blue-600">{totalPending}</div>
		</div>

		<div class="card">
			<div class="text-sm text-gray-600 mb-2">Processing Tasks</div>
			<div class="text-3xl font-bold text-yellow-600">{totalProcessing}</div>
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
		<!-- Queues Grid -->
		<div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
			{#each queues as queue}
				<div class="card hover:shadow-lg transition-shadow">
					<div class="flex items-center justify-between mb-4">
						<h3 class="text-lg font-semibold text-gray-900">{queue.name}</h3>
						<button
							on:click={() => purgeQueue(queue.name)}
							class="text-xs text-red-600 hover:text-red-800 font-medium"
						>
							Purge
						</button>
					</div>

					<div class="grid grid-cols-2 gap-4 mb-4">
						<div>
							<div class="text-xs text-gray-500">Pending</div>
							<div class="text-2xl font-bold text-blue-600">{queue.pending}</div>
						</div>
						<div>
							<div class="text-xs text-gray-500">Processing</div>
							<div class="text-2xl font-bold text-yellow-600">{queue.processing}</div>
						</div>
						<div>
							<div class="text-xs text-gray-500">Consumers</div>
							<div class="text-2xl font-bold text-purple-600">{queue.consumers}</div>
						</div>
						<div>
							<div class="text-xs text-gray-500">Throughput</div>
							<div class="text-2xl font-bold text-green-600">
								{queue.throughput.toFixed(1)}<span class="text-sm text-gray-500">/s</span>
							</div>
						</div>
					</div>

					<!-- Progress Bar -->
					<div class="w-full bg-gray-200 rounded-full h-2">
						<div
							class="bg-blue-600 h-2 rounded-full transition-all"
							style="width: {Math.min((queue.pending / (queue.pending + queue.processing + 1)) * 100, 100)}%"
						></div>
					</div>
				</div>
			{/each}
		</div>

		{#if queues.length === 0}
			<div class="card text-center py-12 text-gray-500">No queues found</div>
		{/if}
	{/if}
</div>
