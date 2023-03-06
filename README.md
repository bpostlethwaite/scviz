# SCVIZ

## TODO
- [x] Scope needs to detect rising edge and lock in to a given phase
- [x] Basic FFT handling
- [ ] Basic Time Series handling
- [ ] Rename ArrayView -> AudioBuf
- [ ] AudioBuf as Trait + Defaults
- [ ] Specialization for Raw, FFT, TimeSeries
- [x] PortBuf precompute aggs, fft before unlocking buf
- [ ] Benchmark processing
- [ ] Validate FFT Amplitude and Power Spectrum
- [ ] Use Hanning Window when windowing incoming signal

## Optimizations
- Benchmark `Vec<[f64; 2]>` allocation performance for two or three sizes of `Vec` vs pre-allocating in a separate thread with `into_vec<A>(self: Box<[T], A>) -> Vec<T, A>` and sending over the channel.
