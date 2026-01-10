<script lang="ts">
	import { onMount } from 'svelte';
	import { apiClient, type OverviewStats } from '$lib/api/client';
	import StatCard from '$lib/components/StatCard.svelte';
	import ThroughputChart from '$lib/components/ThroughputChart.svelte';
	import TaskStatesChart from '$lib/components/TaskStatesChart.svelte';

	let overview: OverviewStats | null = null;
	let loading = true;
	let error: string | null = null;

	async function loadOverview() {
		try {
			loading = true;
			error = null;
			overview = await apiClient.getOverview();
		} catch (e: any) {
			error = e.message;
			console.error('Failed to load overview:', e);
		} finally {
			loading = false;
		}
	}

	onMount(() => {
		loadOverview();

		// Refresh every 10 seconds
		const interval = setInterval(loadOverview, 10000);

		return () => clearInterval(interval);
	});

	$: successRate = overview
		? overview.tasks.success / (overview.tasks.success + overview.tasks.failed) * 100
		: 0;
</script>

<svelte:head>
	<title>Turbine Dashboard - Overview</title>
</svelte:head>

<div class="p-8">
	<div class="mb-8">
		<h1 class="text-3xl font-bold text-gray-900">Dashboard Overview</h1>
		<p class="text-gray-600 mt-2">Real-time monitoring of your Turbine task queue</p>
	</div>

	{#if loading && !overview}
		<div class="flex items-center justify-center h-64">
			<div class="animate-spin rounded-full h-12 w-12 border-b-2 border-primary-600"></div>
		</div>
	{:else if error}
		<div class="bg-red-50 border border-red-200 rounded-lg p-4 text-red-800">
			<p class="font-medium">Error loading dashboard</p>
			<p class="text-sm mt-1">{error}</p>
			<button on:click={loadOverview} class="mt-3 btn-primary text-sm"> Retry </button>
		</div>
	{:else if overview}
		<!-- Stats Cards -->
		<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6 mb-8">
			<StatCard
				title="Pending Tasks"
				value={overview.tasks.pending}
				subtitle="Waiting in queues"
				trend={overview.tasks.pending > 100 ? 'up' : 'stable'}
				color="blue"
			/>

			<StatCard
				title="Running Tasks"
				value={overview.tasks.running}
				subtitle="Currently processing"
				color="yellow"
			/>

			<StatCard
				title="Success Rate"
				value={`${successRate.toFixed(1)}%`}
				subtitle="Task completion rate"
				trend={successRate > 95 ? 'up' : 'down'}
				color={successRate > 95 ? 'green' : 'red'}
			/>

			<StatCard
				title="Active Workers"
				value={overview.workers.active}
				subtitle="Out of {overview.workers.total} total"
				color="purple"
			/>
		</div>

		<!-- Charts -->
		<div class="grid grid-cols-1 lg:grid-cols-2 gap-6 mb-8">
			<div class="card">
				<h2 class="text-lg font-semibold mb-4">Task Throughput</h2>
				<ThroughputChart />
			</div>

			<div class="card">
				<h2 class="text-lg font-semibold mb-4">Task States</h2>
				<TaskStatesChart
					success={overview.tasks.success}
					failed={overview.tasks.failed}
					pending={overview.tasks.pending}
					running={overview.tasks.running}
				/>
			</div>
		</div>

		<!-- Queue Summary -->
		<div class="card">
			<h2 class="text-lg font-semibold mb-4">Queue Summary</h2>
			<div class="grid grid-cols-3 gap-4 text-center">
				<div>
					<div class="text-3xl font-bold text-primary-600">
						{overview.queues.total}
					</div>
					<div class="text-sm text-gray-600 mt-1">Total Queues</div>
				</div>
				<div>
					<div class="text-3xl font-bold text-green-600">
						{overview.queues.active}
					</div>
					<div class="text-sm text-gray-600 mt-1">Active Queues</div>
				</div>
				<div>
					<div class="text-3xl font-bold text-blue-600">
						{overview.throughput.tasks_per_second.toFixed(1)}
					</div>
					<div class="text-sm text-gray-600 mt-1">Tasks/Second</div>
				</div>
			</div>
		</div>
	{/if}
</div>
