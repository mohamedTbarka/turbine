<script lang="ts">
	import '../app.css';
	import { page } from '$app/stores';
	import { events } from '$lib/stores/events';
	import { onMount, onDestroy } from 'svelte';

	onMount(() => {
		events.connect();
	});

	onDestroy(() => {
		events.disconnect();
	});

	const navItems = [
		{ path: '/', label: 'Overview', icon: '📊' },
		{ path: '/tasks', label: 'Tasks', icon: '📋' },
		{ path: '/queues', label: 'Queues', icon: '📬' },
		{ path: '/workers', label: 'Workers', icon: '⚙️' },
		{ path: '/dlq', label: 'DLQ', icon: '⚠️' },
		{ path: '/metrics', label: 'Metrics', icon: '📈' }
	];

	$: currentPath = $page.url.pathname;
</script>

<div class="min-h-screen flex">
	<!-- Sidebar -->
	<aside class="w-64 bg-gray-900 text-white flex flex-col">
		<div class="p-6 border-b border-gray-700">
			<h1 class="text-2xl font-bold text-primary-400">Turbine</h1>
			<p class="text-sm text-gray-400 mt-1">Task Queue Dashboard</p>
		</div>

		<nav class="flex-1 p-4 space-y-2">
			{#each navItems as item}
				<a
					href={item.path}
					class="flex items-center px-4 py-3 rounded-lg transition-colors {currentPath === item.path
						? 'bg-primary-600 text-white'
						: 'text-gray-300 hover:bg-gray-800'}"
				>
					<span class="text-xl mr-3">{item.icon}</span>
					<span class="font-medium">{item.label}</span>
				</a>
			{/each}
		</nav>

		<div class="p-4 border-t border-gray-700">
			<div class="flex items-center space-x-2">
				<div class="w-2 h-2 rounded-full {$events.connected ? 'bg-green-500' : 'bg-red-500'}"></div>
				<span class="text-sm text-gray-400">
					{$events.connected ? 'Connected' : 'Disconnected'}
				</span>
			</div>
		</div>
	</aside>

	<!-- Main Content -->
	<main class="flex-1 overflow-auto">
		<slot />
	</main>
</div>
