// Import this before 'vite', 'vitest' or 'oxlint'. oxlint uses the same allocator as Rolldown.
//
// Rolldown's allocator asks Linux for transparent huge pages. When the memory of the machine is
// fragmented or under pressure, the kernel compacts memory on each of those page faults. A build,
// a test run or a cold start of the dev server then spends most of its time in the kernel.
// MIMALLOC_ALLOW_THP=0 stops the request. The allocator reads it when the Rolldown binding loads,
// which is before Vite reads a config file, so a config file cannot set it.
if (process.platform === 'linux') process.env.MIMALLOC_ALLOW_THP ??= '0';
