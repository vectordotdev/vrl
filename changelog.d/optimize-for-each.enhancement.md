Improve `for_each` performance and reduce memory allocations by iterating over collections directly, binding only the closure parameters that are used, and reusing compiler variable slots across iterations. Benchmarks show a 23–41% throughput improvement for arrays and objects.

authors: jimmystewpot
