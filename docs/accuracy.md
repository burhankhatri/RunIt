# How close the predictions are

Measured on a MacBook Air (M4, 16 GB, fanless) on 2026-11-23 with `runit check`, resting 45 s before each model. The predictor saw only the first 96 KB of each file, as it does over the network.

| Model | Kind | Predicted tok/s | Measured tok/s | Difference |
|---|---|---|---|---|
| Qwen2.5 1.5B Instruct Q4_K_M | dense | 84.4 | 81.8 | +3.2% |
| Llama 3.2 3B Instruct Q4_K_M | dense | 47.5 | 44.2 | +7.3% |
| LFM2.5 8B A1B Q4_K_M | mixture of experts | 90.7 | 77.6 | +16.9% |

On a Windows CI machine (AMD EPYC, processor only), LFM2.5 230M Q4_K_M was predicted at 203 tok/s and measured at 217 (-6.5%).

The range shown in the app is ±15% for dense models that fit, ±25% for mixtures, ±30% when a model is split between graphics and system memory, and ±35% when the header could not be read.

## What this does not cover

- **Three models on one Mac and one on a Windows processor.** Nothing has been measured yet on a discrete Windows graphics card, or with a model split across memories. Treat those as less certain.
- **Heat.** The same Mac, after an hour of steady load and while macOS reported thermal state "fair", ran Llama 3.2 3B and Granite 4.2 3B at 8 to 16 tok/s instead of about 41 to 47. The test measures 20 seconds of steady work (it kept 87% then) and RunIt labels measurements taken while the Mac is hot, but a long hot session can be several times slower than the headline figure.
- **Prompt reading** is within about 13% for the dense models and 35% too fast for the mixture.
