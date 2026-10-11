/* Live pressure estimates are deliberately separate from static heap topology. */
static int budget_query(ogpu_next_device *device) {
    int result = EXIT_FAILURE;
    ogpu_next_memory_budget *budgets = NULL;
    ogpu_next_memory_heap_info *heaps = NULL;
    _Static_assert(sizeof(ogpu_next_memory_budget) == 24, "memory budget ABI");
    ogpu_next_query q = query(OGPU_NEXT_QUERY_MEMORY_BUDGETS,NULL,0,sizeof(*budgets));
    q.count = 91;
    ogpu_next_status status = ogpu_next_memory_budget_query(device,&q);
    if (status == OGPU_NEXT_UNSUPPORTED) {
        REQUIRE(q.count == 91);
        printf("Live memory budgets NOT exercised: native query unavailable.\n");
        result = EXIT_SUCCESS; goto cleanup;
    }
    REQUIRE(status == OGPU_NEXT_OK && q.count > 0);
    uint32_t count = q.count;
    budgets = calloc(count,sizeof(*budgets)); heaps = calloc(count,sizeof(*heaps));
    REQUIRE(budgets && heaps);
    q = query(OGPU_NEXT_QUERY_MEMORY_HEAPS,heaps,count,sizeof(*heaps)); TRY(ogpu_next_device_query(device,&q));
    REQUIRE(q.count == count);
    q = query(OGPU_NEXT_QUERY_MEMORY_BUDGETS,budgets,count,sizeof(*budgets));
    if (count > 1) {
        q.capacity = count-1;
        REQUIRE(ogpu_next_memory_budget_query(device,&q) == OGPU_NEXT_CAPACITY && q.count == count);
        for (uint32_t i = 0; i < count; ++i) REQUIRE(budgets[i].budget == 0 && budgets[i].usage == 0);
        q.capacity = count;
    }
    for (uint32_t sample = 0; sample < 2; ++sample) {
        TRY(ogpu_next_memory_budget_query(device,&q)); REQUIRE(q.count == count);
        for (uint32_t i = 0; i < count; ++i) {
            REQUIRE(budgets[i].heap == heaps[i].id && budgets[i].reserved == 0);
            REQUIRE(budgets[i].budget > 0 && budgets[i].budget <= heaps[i].size);
            /* No monotonicity, immediate allocation delta or usage <= budget assumption. */
        }
    }
    printf("Live memory budgets pass: %u heaps; explicit repeated sampling, no allocation-delta assumption.\n",count);
    result = EXIT_SUCCESS;
cleanup:
    free(budgets); free(heaps); return result;
}
