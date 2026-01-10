<script lang="ts">
	import { onMount } from 'svelte';
	import { apiClient } from '$lib/api/client';

	let metrics = '';
	let loading = true;
	let error: string | null = null;
	let filter = '';

	async function loadMetrics() {
		try {
			loading = true;
			error = null;
			metrics = await apiClient.getMetrics();
		} catch (e: any) {
			error = e.message;
			console.error('Failed to load metrics:', e);
		} finally {
			loading = false;
		}
	}

	onMount(() => {
		loadMetrics();
	});

	$: filteredMetrics = filter
		? metrics
				.split('\n')
				.filter((line) => line.toLowerCase().includes(filter.toLowerCase()))
				.join('\n')
		: metrics;

	$: metricLines = filteredMetrics.split('\n').filter((line) => !line.startsWith('#') && line.trim());
	$: commentLines = filteredMetrics.split('\n').filter((line) => line.startsWith('#'));
</script>

<svelte:head>
	<title>Metrics - Turbine Dashboard</title>
</svelte:head>

<div class="p-8">
	<div class="mb-8">
		<h1 class="text-3xl font-bold text-gray-900">Prometheus Metrics</h1>
		<p class="text-gray-600 mt-2">Raw metrics endpoint data</p>
	</div>

	<!-- Search -->
	<div class="card mb-6">
		<input
			type="text"
			bind:value={filter}
			placeholder="Filter metrics..."
			class="w-full px-4 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-primary-500 focus:border-transparent"
		/>
	</div>

	<!-- Stats -->
	<div class="grid grid-cols-1 md:grid-cols-3 gap-6 mb-6">
		<div class="card">
			<div class="text-sm text-gray-600 mb-2">Total Metrics</div>
			<div class="text-3xl font-bold text-primary-600">{metricLines.length}</div>
		</div>

		<div class="card">
			<div class="text-sm text-gray-600 mb-2">Metric Types</div>
			<div class="text-3xl font-bold text-blue-600">{commentLines.length}</div>
		</div>

		<div class="card">
			<div class="text-sm text-gray-600 mb-2">Endpoint</div>
			<div class="text-sm font-mono mt-2">/api/metrics</div>
			<button on:click={loadMetrics} class="btn-primary mt-3 text-sm"> Refresh </button>
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
		<!-- Metrics Display -->
		<div class="card">
			<div class="bg-gray-900 rounded-lg p-4 overflow-auto max-h-[600px]">
				<pre class="text-sm text-green-400 font-mono">{filteredMetrics}</pre>
			</div>
		</div>

		<!-- Quick Links -->
		<div class="mt-6 card">
			<h3 class="text-sm font-medium text-gray-700 mb-3">Quick Links</h3>
			<div class="flex flex-wrap gap-2">
				<button
					on:click={() => (filter = 'turbine_tasks_total')}
					class="text-xs px-3 py-1 bg-gray-100 hover:bg-gray-200 rounded"
				>
					Total Tasks
				</button>
				<button
					on:click={() => (filter = 'turbine_queue')}
					class="text-xs px-3 py-1 bg-gray-100 hover:bg-gray-200 rounded"
				>
					Queue Metrics
				</button>
				<button
					on:click={() => (filter = 'turbine_worker')}
					class="text-xs px-3 py-1 bg-gray-100 hover:bg-gray-200 rounded"
				>
					Worker Metrics
				</button>
				<button
					on:click={() => (filter = 'duration')}
					class="text-xs px-3 py-1 bg-gray-100 hover:bg-gray-200 rounded"
				>
					Duration Metrics
				</button>
				<button
					on:click={() => (filter = '')}
					class="text-xs px-3 py-1 bg-primary-100 hover:bg-primary-200 text-primary-800 rounded"
				>
					Clear Filter
				</button>
			</div>
		</div>
	{/if}
</div>
