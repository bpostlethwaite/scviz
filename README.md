# SCVIZ

## TODO
- Scope needs to detect rising edge and lock in to a given phase
- Scope should sample multiples of the win_size, so that near a given frequency there is more stability.

## Optimizations
- Benchmark `Vec<[f64; 2]>` allocation performance for two or three sizes of `Vec` vs pre-allocating in a separate thread with `into_vec<A>(self: Box<[T], A>) -> Vec<T, A>` and sending over the channel.
