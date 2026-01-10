<script lang="ts">
	import { onMount } from 'svelte';
	import { apiClient, type TaskInfo } from '$lib/api/client';
	import TaskRow from '$lib/components/TaskRow.svelte';
	import TaskModal from '$lib/components/TaskModal.svelte';

	let tasks: TaskInfo[] = [];
	let loading = true;
	let error: string | null = null;

	// Filters
	let stateFilter = '';
	let queueFilter = '';
	let searchQuery = '';

	// Pagination
	let limit = 50;
	let offset = 0;
	let total = 0;

	// Modal
	let selectedTask: TaskInfo | null = null;
	let showModal = false;

	async function loadTasks() {
		try {
			loading = true;
			error = null;

			const params: any = { limit, offset };

			if (stateFilter) params.state = stateFilter;
			if (queueFilter) params.queue = queueFilter;

			const response = await apiClient.getTasks(params);

			tasks = response.tasks;
			total = response.total;
		} catch (e: any) {
			error = e.message;
			console.error('Failed to load tasks:', e);
		} finally {
			loading = false;
		}
	}

	function handleTaskClick(task: TaskInfo) {
		selectedTask = task;
		showModal = true;
	}

	function closeModal() {
		showModal = false;
		selectedTask = null;
	}

	async function revokeTask(taskId: string) {
		if (!confirm('Are you sure you want to revoke this task?')) {
			return;
		}

		try {
			await apiClient.revokeTask(taskId);
			await loadTasks();
		} catch (e: any) {
			alert(`Failed to revoke task: ${e.message}`);
		}
	}

	onMount(() => {
		loadTasks();

		const interval = setInterval(loadTasks, 5000);
		return () => clearInterval(interval);
	});

	$: filteredTasks = tasks.filter((task) => {
		if (searchQuery && !task.task_id.includes(searchQuery) && !task.task_name.includes(searchQuery)) {
			return false;
		}
		return true;
	});

	$: currentPage = Math.floor(offset / limit) + 1;
	$: totalPages = Math.ceil(total / limit);
</script>

<svelte:head>
	<title>Tasks - Turbine Dashboard</title>
</svelte:head>

<div class="p-8">
	<div class="mb-8">
		<h1 class="text-3xl font-bold text-gray-900">Tasks</h1>
		<p class="text-gray-600 mt-2">View and manage task execution</p>
	</div>

	<!-- Filters -->
	<div class="card mb-6">
		<div class="grid grid-cols-1 md:grid-cols-4 gap-4">
			<input
				type="text"
				bind:value={searchQuery}
				placeholder="Search by ID or name..."
				class="px-4 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-primary-500 focus:border-transparent"
			/>

			<select
				bind:value={stateFilter}
				on:change={loadTasks}
				class="px-4 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-primary-500"
			>
				<option value="">All States</option>
				<option value="pending">Pending</option>
				<option value="running">Running</option>
				<option value="success">Success</option>
				<option value="failed">Failed</option>
				<option value="retry">Retry</option>
			</select>

			<select
				bind:value={queueFilter}
				on:change={loadTasks}
				class="px-4 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-primary-500"
			>
				<option value="">All Queues</option>
				<option value="default">default</option>
				<option value="emails">emails</option>
				<option value="processing">processing</option>
			</select>

			<button on:click={loadTasks} class="btn-primary"> Refresh </button>
		</div>
	</div>

	{#if loading && tasks.length === 0}
		<div class="flex items-center justify-center h-64">
			<div class="animate-spin rounded-full h-12 w-12 border-b-2 border-primary-600"></div>
		</div>
	{:else if error}
		<div class="bg-red-50 border border-red-200 rounded-lg p-4 text-red-800">
			Error: {error}
		</div>
	{:else}
		<!-- Tasks Table -->
		<div class="card overflow-hidden">
			<div class="overflow-x-auto">
				<table class="min-w-full divide-y divide-gray-200">
					<thead class="bg-gray-50">
						<tr>
							<th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase">
								Task ID
							</th>
							<th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase">
								Name
							</th>
							<th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase">
								State
							</th>
							<th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase">
								Queue
							</th>
							<th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase">
								Duration
							</th>
							<th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase">
								Created
							</th>
							<th class="px-6 py-3 text-right text-xs font-medium text-gray-500 uppercase">
								Actions
							</th>
						</tr>
					</thead>
					<tbody class="bg-white divide-y divide-gray-200">
						{#each filteredTasks as task}
							<TaskRow {task} on:click={() => handleTaskClick(task)} on:revoke={() => revokeTask(task.task_id)} />
						{/each}
					</tbody>
				</table>
			</div>

			{#if filteredTasks.length === 0}
				<div class="text-center py-12 text-gray-500">No tasks found</div>
			{/if}
		</div>

		<!-- Pagination -->
		{#if totalPages > 1}
			<div class="flex items-center justify-between mt-6">
				<div class="text-sm text-gray-600">
					Showing {offset + 1} to {Math.min(offset + limit, total)} of {total} tasks
				</div>

				<div class="flex space-x-2">
					<button
						on:click={() => {
							offset = Math.max(0, offset - limit);
							loadTasks();
						}}
						disabled={offset === 0}
						class="btn-secondary disabled:opacity-50"
					>
						Previous
					</button>

					<span class="px-4 py-2 text-sm">
						Page {currentPage} of {totalPages}
					</span>

					<button
						on:click={() => {
							offset = offset + limit;
							loadTasks();
						}}
						disabled={offset + limit >= total}
						class="btn-secondary disabled:opacity-50"
					>
						Next
					</button>
				</div>
			</div>
		{/if}
	{/if}
</div>

{#if showModal && selectedTask}
	<TaskModal task={selectedTask} on:close={closeModal} />
{/if}
