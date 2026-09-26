# PolyAgent Desktop - Deep Research Catalog

**Research date:** 14 August 2026  
**Project examined:** *PolyAgent Desktop: AI Context Bundler + Multi-Agent Orchestration System*  
**Scope:** repository-context bundling, generated project context, code-aware retrieval, local/cloud model routing, local inference, agent orchestration, persistent memory, desktop delivery, security, and optional GUI/voice features.

> **Important limitation.** No report can enumerate *every* paper or GitHub repository forever: both change daily and terminology is inconsistent. This is a curated, reproducible, high-coverage catalog of the most relevant primary papers, benchmarks, official implementations, direct competitors, and implementation references found through 14 August 2026. It deliberately prioritizes sources that could change PolyAgent's architecture or evaluation plan.

## 1. Executive conclusion

PolyAgent currently combines three products:

1. an offline repository-to-prompt/context bundler;
2. a managed local-model desktop runtime; and
3. a multi-agent coding workspace with project memory.

Each area already has strong open-source coverage. The most credible research and product contribution is therefore **not simply “bundle a repository for an LLM.”** It is a **local-first, versioned, inspectable context-control layer** that answers this research question:

> At equivalent repository-task quality, can a versioned hybrid project context reduce actual input tokens, latency, and cost compared with raw bundles, repository maps, lexical retrieval, and vector RAG?

The proposed `PROJECT_CONTEXT.md` should be a **cache/index with provenance**, not a replacement for source code. It needs source links, file/content hashes, commit/ref information, freshness state, and a source-on-demand path for any claim that needs exact evidence.

### Direct competitors for the Phase-1 bundler

- [Repomix](https://github.com/yamadashy/repomix)
- [Code2Prompt](https://github.com/mufeedvh/code2prompt)
- [Gitingest](https://github.com/coderamp-labs/gitingest)
- [files-to-prompt](https://github.com/simonw/files-to-prompt)

These already cover significant parts of the proposed Phase-1 functionality: ignore-aware traversal, formatted bundles, token counts, Git context, selection/filtering, and in some cases secret checks and MCP exposure. PolyAgent needs a clearly stronger position than raw concatenation.

### Recommended product position

**“A local-first context-control plane for coding agents: fresh, source-traceable, inspectable project context that works across cloud and local models.”**

The defensible differences are:

- deterministic AST/symbol/API inventory plus optional LLM-written narrative;
- source-cited and hash-versioned `PROJECT_CONTEXT` sections;
- dependency-aware incremental invalidation after code changes;
- task-specific source-on-demand retrieval, rather than injecting one static summary;
- visible selection reasoning, token/cost/latency accounting, and cache state;
- secret review and explicit approval before project data leaves the device;
- interoperability with `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, and Copilot instruction files instead of inventing another isolated instruction format.

## 2. Corrections to validate before implementation

| Proposal claim | Research/engineering correction | Recommended treatment |
|---|---|---|
| “Never pass raw source files to an LLM.” | Generated summaries can omit detail, become stale, or contain hallucinations. Exact code is necessary for many bug fixes and API-contract questions. | Use a hybrid: global context map + targeted source tool + evidence citations. |
| “60–85% token saving.” | It is a hypothesis, not a general fact. Fewer prompt tokens can reduce quality or increase retries/tool calls. | Benchmark actual billed token use, latency, and task success versus baselines. |
| `cl100k_base` is “Claude-compatible.” | `tiktoken-rs` is for OpenAI tokenization. Claude, Gemini, Mistral, Llama/GGUF differ. Even provider count endpoints may be estimates. | Label it “OpenAI-family estimate”; use target-provider/server tokenization when available. |
| Same cloud/local agent code works through an OpenAI-compatible URL. | `llama.cpp`, Ollama, LM Studio, and hosted providers differ in tool calling, JSON schema, streaming, vision, embeddings, and token counting. | Add provider capability profiles and startup probes; feature-gate unsupported actions. |
| A multi-agent system should be built early. | Simple localization–repair–validation workflows can outperform unnecessarily complex teams. | Establish a measured single-agent baseline first; add roles only for demonstrated value. |
| Regex secret scrubbing makes cloud upload safe. | Secret detectors have false positives/negatives, and repositories can include prompt injection in code/comments/docs. | Scan + user review + scoped export policy; do not treat redaction as a hard security boundary. |

## 3. Academic literature - highest-priority direct papers

### A. Repository documentation, project context, and incremental maintenance

| Paper | Authors / venue | Why it matters to PolyAgent |
|---|---|---|
| [RepoAgent: An LLM-based Tool for Repository-level Code Documentation Generation](https://arxiv.org/abs/2402.16667) | Luo et al., 2024 | Closest direct prior work to a Documenter Agent that creates and maintains repository documentation. Read before defining `PROJECT_CONTEXT.md`. |
| [Supporting Software Maintenance with Dynamically Generated Document Hierarchies](https://arxiv.org/abs/2408.05829) | Dearstyne, Rodriguez, Cleland-Huang, ICSME 2024 | Direct precedent for hierarchical, selectively loaded maintenance documentation. |
| [CodeWiki: Scaling Repository-level Code Documentation Generation](https://arxiv.org/abs/2510.24428) | Pham et al., Findings ACL 2026 | Recursive documentation agents and a multi-level evaluation benchmark; useful comparison point for any “complete codebase summary” claim. |
| [RepoDoc: Knowledge Graph-Driven Repository Documentation with Incremental Updates](https://arxiv.org/abs/2604.26523) | Xu et al., 2026 preprint | Very close to the planned incremental update and dependency-impact feature. Treat as emerging prior art, not a proven production baseline. |
| [RepoSummary: Feature-Oriented Repository Documentation with Traceability](https://arxiv.org/abs/2510.11039) | Zhu et al., 2025 | Relevant to task/feature-focused context rather than only file-by-file summaries. |
| [CodePlan: Repository-Level Coding Using Large Language Models and Planning](https://arxiv.org/abs/2309.12499) | Bairi et al., FSE 2024 | Dependency/change-impact planning is useful for deciding which summary sections become stale after a commit. |
| [RepoUnderstander: A Top-Down Repository Knowledge Graph for LLM Agents](https://arxiv.org/abs/2406.01422) | Ma et al., 2024 | Supports top-down structure plus targeted exploration instead of raw concatenation. |
| [Hierarchical Repository-Level Code Summarization Using Local LLMs](https://arxiv.org/abs/2501.07857) | Dhulshette, Shah, Kulkarni, LLM4Code@ICSE 2025 | Strong local-model precedent for AST -> function -> file -> package summaries. |

### B. Repository structure, code maps, and contextual retrieval

| Paper | Authors / venue | Why it matters |
|---|---|---|
| [RepoGraph: Enhancing AI Software Engineering with Repository-level Code Graph](https://arxiv.org/abs/2410.14684) | Ouyang et al., ICLR 2025 | Strong evidence for a repository-wide code graph as a plug-in navigation/retrieval layer. |
| [GraphCoder: Repository-level Code Completion with Dependency Graphs](https://arxiv.org/abs/2406.07003) | Liu et al., 2024 | Coarse-to-fine retrieval from code dependency/context graphs. |
| [RepoCoder: Repository-Level Code Completion with Iterative Retrieval and Generation](https://arxiv.org/abs/2303.12570) | Zhang et al., 2023 | Shows why context acquisition should be iterative and task-specific, not simply one raw bundle. |
| [Repoformer: Selective Retrieval for Repository-Level Code Completion](https://arxiv.org/abs/2403.10059) | Wu et al., 2024 | Direct challenge to “always retrieve/search”: it makes retrieval conditional. |
| [AutoCodeRover: Autonomous Program Improvement with AST-Aware Search](https://arxiv.org/abs/2404.05427) | Zhang et al., 2024 | Supports AST-aware repository exploration over flat file handling. |
| [Aider Repository Map documentation](https://aider.chat/docs/repomap.html) | Practical implementation reference | A high-value non-paper baseline: ranked symbol-level map using repository relations/PageRank ideas. |

### C. Context compression and long-context reliability

| Paper | Authors / venue | Why it matters |
|---|---|---|
| [LLMLingua: Compressing Prompts for Accelerated Inference of Large Language Models](https://aclanthology.org/2023.emnlp-main.825/) | Jiang et al., EMNLP 2023 | Core prompt-compression baseline; reports up to 20x compression in its setting. Do not generalize the result to code without evaluation. |
| [LongLLMLingua: Accelerating and Enhancing LLMs in Long Context Scenarios via Prompt Compression](https://aclanthology.org/2024.acl-long.91/) | Jiang et al., ACL 2024 | Relevant to section selection/reordering and the “lost in the middle” problem. |
| [Selective Context: Compressing Context to Enhance Inference Efficiency](https://aclanthology.org/2023.emnlp-main.391/) | Li, Dong, Lin, Guerin, EMNLP 2023 | Lightweight low-information pruning baseline; reported lower memory/time with modest quality changes. |
| [RECOMP: Improving Retrieval-Augmented LMs with Context Compression and Selective Augmentation](https://arxiv.org/abs/2310.04408) | Xu, Shi, Choi, ICLR 2024 | Relevant because its compressor may return *no retrieved context* if it does not add value. |
| [LLMLingua-2: Data Distillation for Efficient Prompt Compression](https://arxiv.org/abs/2403.12968) | Pan et al., 2024 | Faster extractive compressor candidate for a future local prefilter. |
| [AutoCompressors: Adapting Language Models to Compress Long Contexts](https://aclanthology.org/2023.emnlp-main.232/) | Chevalier et al., EMNLP 2023 | Model-specific soft-summary-vector research alternative; less portable than Markdown. |
| [ICAE: In-Context Autoencoder for Context Compression](https://arxiv.org/abs/2307.06945) | Ge et al., ICLR 2024 | Memory-slot alternative for a future model-specific memory layer. |
| [Lost in the Middle: How Language Models Use Long Contexts](https://arxiv.org/abs/2307.03172) | Liu et al., 2024 | Essential explanation for why a large context window does not guarantee effective repository understanding. |
| [LongBench](https://arxiv.org/abs/2308.14508) | Bai et al., 2023 | Broad long-context benchmark. Useful for testing ordering and context-window assumptions. |
| [RULER](https://github.com/NVIDIA/RULER) | NVIDIA | Effective-context benchmark implementation useful for validating 8K/long-context local-model behavior. |

### D. Repository-level evaluation and code-agent outcome benchmarks

| Paper / benchmark | Authors / venue | What it measures |
|---|---|---|
| [RepoBench](https://arxiv.org/abs/2306.03091) | Liu, Xu, McAuley, ICLR 2024 | Repository retrieval, cross-file code completion, and end-to-end pipeline evaluation. |
| [CrossCodeEval](https://arxiv.org/abs/2310.11248) | Ding et al., NeurIPS Datasets & Benchmarks 2023 | Multilingual cross-file completion: Python, Java, TypeScript, C#. Excellent for file-selection quality. |
| [RepoQA](https://arxiv.org/abs/2406.06025) | Liu et al., 2024 | Long-context code understanding/search across 50 repositories and 5 languages. |
| [CodeRAG-Bench](https://aclanthology.org/2025.findings-naacl.176/) | Wang et al., Findings NAACL 2025 | RAG for code generation, including repository-level tasks; useful for comparing retrieval quality vs generation quality. |
| [SWE-bench](https://arxiv.org/abs/2310.06770) | Jimenez et al., ICLR 2024 | Real GitHub issue resolution. Strong downstream task-success benchmark. |
| [SWE-agent](https://arxiv.org/abs/2405.15793) | Yang et al., NeurIPS 2024 | Shows the agent-computer interface affects repository navigation/edit/test success. |
| [Agentless](https://arxiv.org/abs/2407.01489) | Xia et al., 2024 | Important control baseline: simple localization/repair/validation can compete with more elaborate agents. |
| [Multi-SWE-bench](https://arxiv.org/abs/2504.02605) | Zan et al., 2025 | Multi-language issue-resolution instances, including Rust and TypeScript. |
| [RepoClassBench](https://github.com/microsoft/repoclassbench) | Microsoft, 2024 | Class-level generation in real repositories; useful additional validation set. |
| [Agent Retrieval Bench](https://arxiv.org/abs/2607.24882) | Qin, Xie, 2026 preprint | Directly targets file-level context acquisition; useful emerging benchmark for the “task box” feature. |

### E. Orchestration, tools, planning, validation, and multi-agent design

| Paper | Authors / venue | Why it matters |
|---|---|---|
| [ReAct: Synergizing Reasoning and Acting in Language Models](https://arxiv.org/abs/2210.03629) | Yao et al., ICLR 2023 | Foundation for an orchestrator that alternates thought and tool use. |
| [Reflexion](https://arxiv.org/abs/2303.11366) | Shinn et al., 2023 | Verbal feedback/episodic memory pattern for validator feedback and task history. |
| [AutoGen](https://arxiv.org/abs/2308.08155) | Wu et al., 2023 | Foundational configurable multi-agent conversation framework. |
| [MetaGPT](https://proceedings.iclr.cc/paper_files/paper/2024/hash/6507b115562bb0a305f1958ccc87355a-Abstract-Conference.html) | Hong et al., ICLR 2024 | Role/SOP pipeline and intermediate verification - relevant to explicit Validator stage. |
| [ChatDev](https://aclanthology.org/2024.acl-long.810/) | Qian et al., ACL 2024 | Specialized collaborative software-development agents and communication patterns. |
| [Magentic-One](https://arxiv.org/abs/2411.04468) | Fourney et al., 2024 | General multi-agent orchestrator that plans, tracks, replans, and directs specialized agents. |
| [ToolLLM / ToolBench](https://arxiv.org/abs/2307.16789) | Qin et al., ICLR 2024 | Tool-use data and evaluation; useful whenever Planner accesses external APIs. |
| [API-Bank](https://arxiv.org/abs/2304.08244) | Li et al., EMNLP 2023 | Benchmarking tool/API use with stateful calls. |
| [tau-bench](https://arxiv.org/abs/2406.12045) | Yao et al., 2024 | Stateful tool-agent reliability; useful evidence for validating side-effect tools. |
| [ToolEmu](https://arxiv.org/abs/2309.15817) | Ruan et al., ICLR 2024 | Tool-use risk evaluation/emulation; relevant before agent shell/API capabilities. |
| [AgentBench](https://arxiv.org/abs/2308.03688) | Liu et al., 2023 | Broad interactive agent benchmark. |

### F. RAG, graph memory, and long-lived task history

| Paper | Authors / venue | Why it matters |
|---|---|---|
| [Retrieval-Augmented Generation for Knowledge-Intensive NLP Tasks](https://arxiv.org/abs/2005.11401) | Lewis et al., NeurIPS 2020 | Canonical RAG formulation. |
| [RAPTOR](https://arxiv.org/abs/2401.18059) | Sarthi et al., 2024 | Recursive embed/cluster/summarize tree; strong analogue for hierarchical project summaries. |
| [GraphRAG: From Local to Global](https://arxiv.org/abs/2404.16130) | Edge et al., 2024 | Entity graph + community summaries for global corpus questions. A Phase-4 alternative to flat vector search. |
| [HippoRAG](https://arxiv.org/abs/2405.14831) | Gutiérrez et al., NeurIPS 2024 | Knowledge graph plus personalized PageRank for long-term memory. |
| [LightRAG](https://arxiv.org/abs/2410.05779) | Guo et al., Findings EMNLP 2025 | Incremental graph-plus-vector retrieval; relevant to changed-file updates. |
| [RAGAS](https://arxiv.org/abs/2309.15217) | Es et al., 2023 | Reference-free RAG evaluation metrics including context relevance and faithfulness. |
| [MemGPT](https://arxiv.org/abs/2310.08560) | Packer et al., 2023 | Memory tiers / virtual context-management model. |
| [Generative Agents](https://doi.org/10.1145/3586183.3606763) | Park et al., UIST 2023 | Memory, reflection, and planning pattern. |
| [LongMemEval](https://github.com/xiaowu0162/longmemeval) | Wu et al. | Long-term memory evaluation including updates, temporal reasoning, and abstention. |
| [LongMemEval-V2](https://arxiv.org/abs/2605.12493) | Wu et al., 2026 | Adds long agent trajectories and query latency. Strong fit for Phase-4 task history. |

### G. Model routing, local/on-device inference, and quantization

| Paper | Authors / venue | Why it matters |
|---|---|---|
| [FrugalGPT](https://arxiv.org/abs/2305.05176) | Chen, Zaharia, Zou, TMLR 2024 | Cascades, prompt adaptation, and model approximation for cost-quality tradeoffs. |
| [Hybrid LLM: Cost-Efficient and Quality-Aware Query Routing](https://arxiv.org/abs/2404.14618) | Ding et al., ICLR 2024 | Difficulty-based routing between small/large models. |
| [RouteLLM: Learning to Route LLMs from Preference Data](https://arxiv.org/abs/2406.18665) | Ong et al., ICLR 2025 | Preference-trained router between strong/weak models. |
| [BEST-Route](https://proceedings.mlr.press/v267/ding25d.html) | Ding et al., ICML 2025 | Model and best-of-n allocation based on query difficulty. |
| [LLMRouterBench](https://arxiv.org/abs/2601.07206) | Li et al., 2026 | Large routing benchmark; useful warning that routers should be calibrated rather than trusted automatically. |
| [MobileLLM](https://arxiv.org/abs/2402.14905) | Liu et al., 2024 | Evidence for lightweight local models and API-calling capabilities. |
| [Phi-3 Technical Report](https://arxiv.org/abs/2404.14219) | Abdin et al., 2024 | Small-model deployment reference. |
| [LLM in a Flash](https://arxiv.org/abs/2312.11514) | Alizadeh et al., 2023 | Inference when model parameters exceed device memory. |
| [GPTQ](https://arxiv.org/abs/2210.17323) | Frantar et al., 2022 | Post-training low-bit quantization foundation. |
| [AWQ](https://arxiv.org/abs/2306.00978) | Lin et al., 2023 | Activation-aware low-bit quantization reference. |

### H. Optional GUI-agent, voice, and multilingual research

This cluster only matters if PolyAgent expands beyond a developer desktop app into general computer control or voice interaction. It should **not** be part of a first release.

| Paper | Authors / venue | Relevance |
|---|---|---|
| [OSWorld](https://arxiv.org/abs/2404.07972) | Xie et al., 2024 | Executable benchmark across Ubuntu, Windows, macOS, web/desktop tasks. |
| [UFO](https://arxiv.org/abs/2402.07939) | Zhang et al., 2024 | Windows desktop UI observer/controller architecture. |
| [CogAgent](https://arxiv.org/abs/2312.08914) | Hong et al., 2023 | Screenshot-driven GUI interaction model. |
| [SeeClick + ScreenSpot](https://arxiv.org/abs/2401.10935) | Cheng et al., 2024 | Visual GUI grounding and benchmark. |
| [OmniParser](https://arxiv.org/abs/2408.00203) | Lu et al., 2024 | Detects screen elements and semantics for visual GUI agents. |
| [OS-Atlas](https://arxiv.org/abs/2410.23218) | Wu et al., 2024 | Large cross-platform GUI-element dataset/model work. |
| [UI-TARS](https://arxiv.org/abs/2501.12326) | Qin et al., 2025 | Screenshot-native GUI agent with reasoning/reflection. |
| [MPR-GUI](https://aclanthology.org/2026.acl-long.1375/) | Chen et al., ACL 2026 | Six-language GUI perception/reasoning benchmark; important if claiming multilingual computer use. |
| [MIRACL](https://aclanthology.org/2023.tacl-1.63/) | Zhang et al., TACL 2023 | Multilingual retrieval benchmark for any cross-language RAG claims. |
| [BGE-M3](https://arxiv.org/abs/2402.03216) | Chen et al., 2024 | Multilingual dense/sparse/multi-vector embedding candidate. |

### I. Security papers that are mandatory for this project

Files, READMEs, web pages, PDF text, issues, cached summaries, and tool results are all potentially untrusted inputs. They can try to alter agent behavior through indirect prompt injection.

| Paper | Why it must be included |
|---|---|
| [Not What You've Signed Up For: Compromising Real-World LLM-Integrated Applications with Indirect Prompt Injection](https://arxiv.org/abs/2302.12173) | Core indirect-prompt-injection risk for repository/web/PDF ingestion. |
| [Prompt Injection attack against LLM-integrated Applications](https://arxiv.org/abs/2306.05499) | Practical attack research on LLM-integrated apps. |
| [Formalizing and Benchmarking Prompt Injection Attacks and Defenses](https://arxiv.org/abs/2310.12815) | Framework to assess injection attacks and defenses. |
| [AgentDojo](https://arxiv.org/abs/2406.13352) | Benchmark for prompt injection and tool-use safety. |
| [InjecAgent](https://aclanthology.org/2024.findings-acl.624/) | Tool-integrated agent injection benchmark. |
| [PoisonedRAG](https://arxiv.org/abs/2402.07867) | RAG knowledge-base poisoning risk. |
| [OWASP Top 10 for LLM Applications](https://genai.owasp.org/llm-top-10/) | Practical threat taxonomy; prompt injection is a top category. |
| [OWASP MCP Top 10](https://owasp.org/www-project-mcp-top-10/) | Required if PolyAgent later exposes/consumes MCP tools. |

## 4. GitHub and implementation-resource catalog

### A. Recommended native-Rust/Tauri foundation

| Resource | License / role | Recommendation |
|---|---|---|
| [tauri-apps/tauri](https://github.com/tauri-apps/tauri) | MIT/Apache-2.0; desktop shell | **Adopt.** Matches Rust backend + React UI, cross-platform packaging, permissions, updater. |
| [tauri-apps/plugins-workspace](https://github.com/tauri-apps/plugins-workspace) | MIT/Apache-2.0; official plugins | **Adopt selectively.** Use `fs`, dialog, clipboard, log, updater, etc.; never give the frontend generic shell/process capability. |
| [BurntSushi/ripgrep (`ignore` crate)](https://github.com/BurntSushi/ripgrep) | MIT/Unlicense; ignore-aware traversal | **Adopt.** Correct baseline for `.gitignore`, global excludes, negations, and parallel walk. Regression-test edge cases. |
| [bojand/infer](https://github.com/bojand/infer) | MIT; MIME/magic-byte detection | **Adopt.** Pair with NUL-byte checks, max size, and allow/deny policy. |
| [zurawiki/tiktoken-rs](https://github.com/zurawiki/tiktoken-rs) | MIT; OpenAI token estimator | **Adopt with honest labels.** Do not call the result a universal token count. |
| [rusqlite/rusqlite](https://github.com/rusqlite/rusqlite) | MIT; embedded SQLite | **Adopt.** Use `bundled` for predictable desktop installs; parameterize SQL. |
| [tree-sitter/tree-sitter](https://github.com/tree-sitter/tree-sitter) | MIT; incremental parsing | **Adopt later.** Foundation for symbols/API signatures and safe code-aware context. |
| [ast-grep/ast-grep](https://github.com/ast-grep/ast-grep) | MIT; structural code search | **Evaluate later.** Useful for semantic/structural selection, not necessary for first basic bundler. |
| [notify-rs/notify](https://github.com/notify-rs/notify) | Rust; filesystem watch | **Optional Phase 2.** Useful for granular watch control and incremental invalidation. |
| [open-source-cooperative/keyring-rs](https://github.com/open-source-cooperative/keyring-rs) | MIT/Apache-2.0; native key stores | **Adopt for cloud keys.** Do not put API keys in SQLite, presets, markdown, or logs. |

### B. Closest bundler/context competitors - benchmark, do not copy blindly

| Repository | What it already does | How PolyAgent should use it |
|---|---|---|
| [mufeedvh/code2prompt](https://github.com/mufeedvh/code2prompt) | Rust repository-to-prompt packing, ignore handling, token tracking, templates, Git diffs, MCP/Python interfaces | Closest Rust competitor/reference. Compare bundle correctness, UX, and tests. |
| [yamadashy/repomix](https://github.com/yamadashy/repomix) | Mature packaging, output styles, config, tree/compression, Git context, security checks | Strong behavior/UX benchmark for Phase 1. |
| [coderamp-labs/gitingest](https://github.com/coderamp-labs/gitingest) | Prompt-friendly local/remote Git repository digest and token counts | Baseline for simple repository ingestion. |
| [simonw/files-to-prompt](https://github.com/simonw/files-to-prompt) | Compact Python repository-selection/prompt output tool | Readable baseline for filtering/escaping/output format. |
| [Aider-AI/aider](https://github.com/Aider-AI/aider) | Codebase map, local/cloud models, Git-aware coding | Best conceptual comparison for a repository map. Mine ideas/tests, not its Python runtime. |
| [pdavis68/RepoMapper](https://github.com/pdavis68/RepoMapper) | Standalone Aider-inspired, token-aware code map | Useful smaller reference implementation. |
| [PatrickSys/codebase-context](https://github.com/PatrickSys/codebase-context) | Bounded conventions map, symbol search, MCP resource | Strong current design reference for source-on-demand context. |
| [sibyllinesoft/scribe](https://github.com/sibyllinesoft/scribe) | Dependency-aware, token-budgeted surgical code retrieval | Benchmark/reference for “only send what matters.” |

### C. Documentation, memory, and code-understanding references

| Repository | Use |
|---|---|
| [OpenBMB/RepoAgent](https://github.com/OpenBMB/RepoAgent) | Closest open implementation accompanying repository documentation generation research. |
| [SYSUSELab/RepoDoc](https://github.com/SYSUSELab/RepoDoc) | Emerging graph-based incremental repository documentation reference. |
| [CognitionAI/deepwiki](https://github.com/CognitionAI/deepwiki) | Generated codebase wiki/understanding reference. |
| [AsyncFuncAI/deepwiki-open](https://github.com/AsyncFuncAI/deepwiki-open) | Open alternative / architecture reference for generated codebase wikis. |
| [fockus/skill-memory-bank](https://github.com/fockus/skill-memory-bank) | File-based persistent agent memory patterns. |
| [feelingsonice/MemoryBank](https://github.com/feelingsonice/MemoryBank) | Local shared long-term agent memory with SQLite/embeddings. |
| [GreatScottyMac/context-portal](https://github.com/GreatScottyMac/context-portal) | Project memory-bank MCP design. |
| [letta-ai/letta](https://github.com/letta-ai/letta) | Stateful-agent / MemGPT implementation reference. |
| [letta-ai/letta-code](https://github.com/letta-ai/letta-code) | Memory-first coding-agent product reference. |

### D. Agent orchestration and interoperability

| Repository | Why inspect it | Adoption position |
|---|---|---|
| [0xPlaygrounds/rig](https://github.com/0xPlaygrounds/rig) | Rust provider abstraction, tools, streaming, embeddings/vector-store integrations | **Likely adopt, but hide behind an internal adapter and pin exact versions.** It evolves quickly. |
| [modelcontextprotocol/rust-sdk](https://github.com/modelcontextprotocol/rust-sdk) | Official Rust MCP SDK | **Phase 4+.** Only enable user-approved, allowlisted MCP. |
| [microsoft/autogen](https://github.com/microsoft/autogen) | Multi-agent conversations and human/tool workflows | Concept/reference; Python. |
| [langchain-ai/langgraph](https://github.com/langchain-ai/langgraph) | Durable graph orchestration and human-in-the-loop semantics | Concept/reference; do not add a Python sidecar merely to replicate it. |
| [crewAIInc/crewAI](https://github.com/crewAIInc/crewAI) | Role-based agent-team design | Concept/reference. |
| [pydantic/pydantic-ai](https://github.com/pydantic/pydantic-ai) | Structured outputs, validation, agent workflows | Implement equivalent `serde`/JSON-Schema validation in Rust rather than embedding Python. |
| [lm-sys/RouteLLM](https://github.com/lm-sys/RouteLLM) | Research router serving/evaluation | Evaluate its routing methods as baselines. |
| [microsoft/best-route-llm](https://github.com/microsoft/best-route-llm) | HybridLLM/BEST-Route research implementations | Routing evaluation/reference. |
| [ynulihao/LLMRouterBench](https://github.com/ynulihao/LLMRouterBench) | Unified routing benchmark | Use to avoid weak routing claims. |
| [BerriAI/litellm](https://github.com/BerriAI/litellm) | Provider gateway/cost tracking | Product reference; Python service is usually too heavy for a native desktop MVP. |

### E. Local inference, models, embedding, and retrieval

| Repository | Why relevant | Recommendation |
|---|---|---|
| [ggml-org/llama.cpp](https://github.com/ggml-org/llama.cpp) | Local GGUF inference; `llama-server`; OpenAI-like API; embeddings/reranking/schema options | **Optional managed runtime after Phase 1.** Capability probe every model; security-isolate it. |
| [ollama/ollama](https://github.com/ollama/ollama) | Widely used local-model endpoint/model manager | Support as a user-provided endpoint first. |
| [EricLBuehler/mistral.rs](https://github.com/EricLBuehler/mistral.rs) | Rust-native inference alternative | Explore only if llama.cpp does not meet concrete needs. |
| [huggingface/candle](https://github.com/huggingface/candle) | Rust ML framework | Future small embedding/STT experiments, not first production inference engine. |
| [huggingface/hf-hub](https://github.com/huggingface/hf-hub) | Rust model-download client | Use only with immutable revision + SHA-256 + explicit license acceptance. |
| [lancedb/lancedb](https://github.com/lancedb/lancedb) | Embedded vector store with metadata, FTS, SQL, Rust SDK | **Recommended Phase-4 storage.** Avoid a server dependency. |
| [Anush008/fastembed-rs](https://github.com/Anush008/fastembed-rs) | Local ONNX embeddings/reranking | Strong pairing with LanceDB; benchmark memory/download/quality. |
| [quickwit-oss/tantivy](https://github.com/quickwit-oss/tantivy) | Rust BM25/full-text indexing | Defer until LanceDB FTS cannot meet requirements; avoid duplicate early indexes. |
| [asg017/sqlite-vec](https://github.com/asg017/sqlite-vec) | Vector search inside SQLite | Attractive but pre-v1 extension/migration/signing risk; do not choose first. |
| [unum-cloud/usearch](https://github.com/unum-cloud/usearch) | Fast ANN index | Only if you want to build persistence/metadata consistency yourself. |

### F. Coding-workspace and local-agent competitors

| Repository | Relevance |
|---|---|
| [Cline/Cline](https://github.com/Cline/Cline) | Multi-provider coding agent, approval gates, tools, MCP, teams, checkpoints, schedules. Strong product competitor/reference. |
| [OpenHands/OpenHands](https://github.com/OpenHands/OpenHands) | General coding-agent workspace reference. |
| [SWE-agent/SWE-agent](https://github.com/SWE-agent/SWE-agent) | Benchmark agent/computer-interface reference. |
| [continuedev/continue](https://github.com/continuedev/continue) | Open coding assistant / local model integrations. |
| [anomalyco/opencode](https://github.com/anomalyco/opencode) | Open coding agent with local/multi-provider/subagent development. |
| [plandex-ai/plandex](https://github.com/plandex-ai/plandex) | Long-running coding-agent/task-context reference. |
| [Aider-AI/aider](https://github.com/Aider-AI/aider) | Relevant here too because it combines maps, local/cloud models, Git, and edits. |

### G. Security, provenance, and release-chain tools

| Repository | Purpose | Caveat |
|---|---|---|
| [gitleaks/gitleaks](https://github.com/gitleaks/gitleaks) | Secret scan before bundle/export/upload | Upstream calls it feature-complete; version detector rules and do not rely on it alone. |
| [secretlint/secretlint](https://github.com/secretlint/secretlint) | Pluggable secret scanner used by Repomix | Add only if a JS/TS scanner is a deliberate supported component. |
| [trufflesecurity/trufflehog](https://github.com/trufflesecurity/trufflehog) | Detect/verify secrets | Useful CI or optional scan alternative; network verification needs an explicit privacy decision. |
| [sigstore/cosign](https://github.com/sigstore/cosign) | Sign/verify release binaries and model manifests by digest | CI/release control; pin digest, not mutable tags. |
| [ossf/scorecard](https://github.com/ossf/scorecard) | Supply-chain assessment | Use in CI for dependencies/release process. |
| [CycloneDX/cyclonedx-rust-cargo](https://github.com/CycloneDX/cyclonedx-rust-cargo) | SBOM generation | Strong addition for desktop sidecars and dependency release process. |

### H. Optional GUI/voice/multilingual resources

| Repository | Role | Recommendation |
|---|---|---|
| [bytedance/UI-TARS-desktop](https://github.com/bytedance/UI-TARS-desktop) | Cross-platform GUI agent reference | Research only; model/data licenses and remote control risk are separate. |
| [microsoft/UFO](https://github.com/microsoft/UFO) | Windows UIA/Win32/COM agent reference | Windows architecture reference, not a portable core dependency. |
| [OpenAdaptAI/openadapt-desktop](https://github.com/OpenAdaptAI/openadapt-desktop) | Tauri + sidecar + consent/review architecture | Study its boundary/consent design, not its beta delivery path. |
| [microsoft/OmniParser](https://github.com/microsoft/OmniParser) | Screen element parsing research | Code and model-weight licenses differ; some weights have restrictive terms. |
| [nut-tree/nut.js](https://github.com/nut-tree/nut.js) | Cross-platform keyboard/mouse/image matching | Accessibility/Screen Recording permissions and packaging terms make it unsuitable as a default. |
| [xlang-ai/OSWorld](https://github.com/xlang-ai/OSWorld) | Cross-OS GUI agent benchmark | Evaluation only. |
| [microsoft/WindowsAgentArena](https://github.com/microsoft/WindowsAgentArena) | Windows GUI benchmark | Evaluation only. |
| [ggml-org/whisper.cpp](https://github.com/ggml-org/whisper.cpp) | Offline STT sidecar | Best optional speech approach; make microphone access explicit/optional. |
| [QwenLM/Qwen3-ASR](https://github.com/QwenLM/Qwen3-ASR) | Multilingual ASR research | Heavy Python/Torch runtime; not a default desktop dependency. |
| [FlagOpen/FlagEmbedding](https://github.com/FlagOpen/FlagEmbedding) | BGE multilingual embedding baselines | Benchmark model terms/quality before commercial use. |
| [QwenLM/Qwen3-Embedding](https://github.com/QwenLM/Qwen3-Embedding) | Multilingual code/text embeddings | Legal/model-license review required. |

## 5. Standards and official documentation to track

| Standard / official resource | Why it is relevant |
|---|---|
| [AGENTS.md](https://agents.md/) | Open Markdown instructions format. PolyAgent should consume/sync it, not conflict with it. |
| [GitHub Copilot custom instructions](https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/add-custom-instructions) | Shows current agent instruction-file discovery and repository-specific guidance. |
| [Model Context Protocol specification](https://modelcontextprotocol.io/specification/) | Only relevant if adding MCP tools/resources. Treat as a new trust boundary. |
| [Tauri sidecars](https://v2.tauri.app/develop/sidecar/) | Official named-sidecar and argument permission model. |
| [Tauri capabilities](https://v2.tauri.app/security/capabilities/) | Needed to narrowly scope filesystem/process access. |
| [llama.cpp security policy](https://github.com/ggml-org/llama.cpp/security) | Requires treating unknown models as untrusted, avoiding untrusted network exposure, verifying artifacts, and isolating execution. |
| [OWASP Top 10 for LLM Applications](https://genai.owasp.org/llm-top-10/) | Security design and test checklist. |
| [NIST AI Risk Management Framework](https://www.nist.gov/itl/ai-risk-management-framework) | Governance/risk framework useful for a university report and release process. |
| [OpenTelemetry GenAI semantic conventions](https://opentelemetry.io/docs/specs/semconv/registry/attributes/gen-ai/) | Observability format for model, token, tool, routing, and latency traces. |
| [CycloneDX specification](https://cyclonedx.org/specification/overview/) | Software bill of materials for Rust packages, sidecars, and release artifacts. |
| [Git ignore documentation](https://git-scm.com/docs/gitignore) | Authoritative behavioral source for the file-walker rules. |

## 6. Recommended phased product/research plan

### Phase 1 - prove deterministic bundling value

**Job to be done:** “I need a safe, inspectable, high-quality context pack for another coding agent or chat.”

- Native Tauri/Rust shell and a read-only backend.
- `ignore` walker + binary/MIME/NUL checks + size/file/token budgets.
- Interactive selection tree, search, bundle preview, copy/save.
- Output styles, path headers, line-number option, Git-diff mode later.
- Honest token labels: `OpenAI-family estimate`, target-provider/server count when available.
- Before export: secret scan + clear user review, without claiming perfect protection.
- Benchmark behavior against Repomix, Code2Prompt, Gitingest, and files-to-prompt.

**Do not include in the first usable version:** code editing, arbitrary shell execution, web retrieval, generic MCP, auto-download model manager, general GUI control, speech, vector DB, or multi-agent autonomy.

### Phase 2 - context-control differentiator

- AST/symbol extraction with Tree-sitter.
- A canonical internal context model rendered as `PROJECT_CONTEXT.md` for people.
- Each generated claim stores `path`, symbol/range when possible, content hash, commit/ref, generation time, and confidence/freshness.
- Consume `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, and path-specific instruction files; keep agent instructions separate from generated repository facts.
- Dependency graph and change-impact invalidation.
- Show a “stale / regenerated / source-on-demand” status in the UI.

### Phase 3 - optional local/cloud model adapter

- Start with **pluggable local endpoint profiles**: llama.cpp, Ollama, LM Studio, OpenAI-compatible endpoint.
- Capability probe: chat, streaming, tool calls, JSON schema, vision, embeddings, tokenization.
- Store cloud credentials in native keychain only.
- Add measured cost/latency router after gathering task data, not based on static heuristics alone.
- If shipping `llama-server`, bind loopback only, use a per-launch secret, pin and hash-check sidecar/model artifacts, and forbid generic frontend-controlled process arguments.

### Phase 4 - retrieval and persistent memory

- Use LanceDB + a local embedding/reranking spike only if deterministic context/AST selection demonstrably misses relevant evidence.
- Begin with file/symbol metadata, hashes, branch/worktree, and task provenance.
- Add vector/graph retrieval behind evaluation gates; preserve raw source-on-demand.
- Task history must contain date, branch/worktree, source provenance, retention/deletion policy, and superseded/stale state.

### Phase 5 - carefully scoped tools and multi-agent workflows

- Read-only tools first.
- Per-action approval for writes, shell commands, external APIs, model downloads, and MCP servers.
- Validator checks structured output **and** source/commit freshness.
- Add parallel specialists only after comparison against a capable single-agent baseline demonstrates better success/cost/latency.

## 7. Minimum viable research evaluation

Use fixed tasks, fixed budgets, fixed tools, and the same target model wherever possible. Compare:

1. **Raw selected files** - user-selected full source.
2. **Compressed bundle baseline** - Repomix/Code2Prompt-like output.
3. **Lexical/vector RAG** - BM25 and/or dense retrieval.
4. **Repository map / dependency graph** - Aider/RepoGraph-like deterministic structure.
5. **PolyAgent hybrid** - versioned global context + AST map + task retrieval + source-on-demand.

### Measures

| Dimension | Suggested measure |
|---|---|
| Downstream success | Tests passing; SWE-bench/RepoBench-style task resolution. |
| Context selection | Relevant file/symbol recall and precision; use RepoQA/CrossCodeEval-style gold context where possible. |
| Cost | Actual input/output/reasoning/tool tokens and provider bill, not only the length of `PROJECT_CONTEXT.md`. |
| Latency | p50/p95 total latency, time-to-first-token, retrieval/index/update time, retry count. |
| Faithfulness | Sampled documentation claims verified against cited code; RAGAS-like relevance/faithfulness metrics plus human audit. |
| Freshness | Controlled commits: rate of stale sections detected, regenerated, and incorrectly retained. |
| Security | Secret exposure false positive/negative review; AgentDojo/InjecAgent-style injection outcomes. |
| Human control | Can a user inspect, correct, exclude, and understand the exact context sent? |
| Multilingual / cross-platform | CrossCodeEval/MIRACL where scope includes multiple languages; Windows/macOS packaging tests. |

### Required ablations

- prose summary only vs summary + raw-source tool;
- full regeneration vs dependency-aware incremental update;
- AST/repository map vs LLM-only documentation;
- fixed frontier model vs routed cloud/local model;
- cached versus uncached context;
- raw/untrusted content versus injection-hardened content handling.

## 8. Security rules for the architecture document

1. Treat code, documentation, PDFs, browser pages, Git issues, commits, model output, vector results, and MCP output as **data, not instructions**.
2. Keep initial agent tools read-only. Require explicit user confirmation for filesystem writes, shell, Git commit/push, network calls, model download, and external MCP.
3. Scope each filesystem capability to the user-approved project directory, not broad home-directory access.
4. Do not let a web frontend construct arbitrary sidecar commands. Use named sidecars and a narrow Rust-owned argument allowlist.
5. Bind local inference to loopback/local socket only. Do not expose an unauthenticated `llama-server` to a network.
6. Use a per-launch secret for any local HTTP sidecar where feasible and never log it.
7. Verify immutable model/sidecar digests; maintain a signed manifest/SBOM. Repository code licenses and model-weight licenses must be reviewed separately.
8. Use OS native credential storage for cloud keys. Never include values in bundles, history, screenshots, telemetry, or `PROJECT_CONTEXT.md`.
9. Keep audit records for external transmission: which files/sections, model/provider, time, token/cost estimate, and explicit approval.
10. Include prompt-injection and stale-context regression cases in automated tests.

## 9. Search methodology for continuing the catalog

For future updates, record **query, source, access date, language/license filter, last activity, selected commit/release, and inclusion reason**. Search in arXiv, Semantic Scholar, ACL Anthology, OpenReview, ACM DL, IEEE Xplore, GitHub, Papers with Code, and official vendor documentation.

### Query families

```text
repository-level code completion context selection retrieval benchmark
repository documentation generation incremental update knowledge graph
codebase context compression LLM source code
project context summarization source traceability stale documentation
cross-file code completion retrieval RepoQA RepoBench CrossCodeEval
code agent repository map dependency graph context
LLM long context effective context benchmark RULER LongBench
LLM routing cost quality coding tasks FrugalGPT RouteLLM
multi-agent coding agent single agent baseline orchestration evaluation
function calling structured output benchmark BFCL
agent tool prompt injection benchmark AgentDojo InjecAgent
MCP security authorization localhost transport
semantic code retrieval language server MCP
local LLM code agent GGUF OpenAI compatibility tool calling
agent memory task history knowledge updates benchmark
source-code secret scanning redaction false-negative evaluation
Tauri sidecar supply chain signing notarization SBOM
```

## 10. Short reading order

If the team only has time for ten resources before making an implementation decision, read these first:

1. [RepoAgent](https://arxiv.org/abs/2402.16667)
2. [CodeWiki](https://aclanthology.org/2026.findings-acl.288/)
3. [RepoGraph](https://arxiv.org/abs/2410.14684)
4. [LLMLingua](https://aclanthology.org/2023.emnlp-main.825/)
5. [RepoBench](https://arxiv.org/abs/2306.03091)
6. [RepoQA](https://arxiv.org/abs/2406.06025)
7. [SWE-bench](https://arxiv.org/abs/2310.06770)
8. [FrugalGPT](https://arxiv.org/abs/2305.05176)
9. [AgentDojo](https://arxiv.org/abs/2406.13352)
10. [Repomix](https://github.com/yamadashy/repomix) and [Code2Prompt](https://github.com/mufeedvh/code2prompt)

## 11. Suggested thesis/proposal language

> PolyAgent is a local-first context-control plane for repository-aware AI development. Rather than assuming that all source code should be bundled or that generated documentation can replace source, PolyAgent maintains a versioned, source-traceable project-context index. It combines deterministic structural analysis, optional model-generated summaries, targeted source retrieval, and measurable model routing. The central evaluation is whether this hybrid context preserves repository-task quality while reducing real token, latency, and privacy costs relative to raw bundles, code maps, and retrieval-only systems.

## 12. AtomGit / Chinese ecosystem scan (14 August 2026)

### Scope and method

This was a direct, public, on-platform sweep of AtomGit rather than a web-search-only list. It used English and Chinese query families, including `智能体` (AI agent), `多智能体` (multi-agent), `大语言模型` (LLM), `RAG`, `MCP`, `代码助手` (coding assistant), `本地模型` (local model), `llama.cpp`, `Tauri`, and `openJiuwen`. The live platform returned, at the time of the scan, 4,402 repository matches for `智能体`, 573 for `RAG`, 74 for `代码助手`, and 97 for `llama.cpp`.

This is a **curated deep scan**, not a claim that every one of AtomGit's hundreds of thousands of public projects has been manually assessed. Search result counts are dynamic and AtomGit sometimes renders an initial zero before the result list hydrates, so the selected projects and their direct links are the reproducible deliverable.

### Provenance rule

AtomGit mixes China-native/primary-looking repositories with GitHub mirrors and dual-hosted projects. Do not treat a repository merely hosted on AtomGit as the canonical source.

- **Primary-looking AtomGit host** means the page showed active work under an AtomGit-focused organization or individual namespace and did not show a mirror notice or external canonical-host link during this scan. It still needs normal provenance, license, commit, and release-artifact review before adoption.
- **Mirror or secondary host** means the page explicitly identifies a GitHub mirror, uses a `GitHub_Trending` or `gh_mirrors` namespace, or links to a canonical GitHub source. Use it for convenient China-hosted discovery/access, but cite and pin the upstream project for product dependencies.

### Highest-value primary-looking AtomGit projects

| Project | Observed public details | Why it matters to PolyAgent | Position |
|---|---|---|---|
| [AtomCode / atomcode](https://atomgit.com/atomgit_atomcode/atomcode) | Rust, MIT; 3,774 stars, 577 forks, 5,472 commits; active issue/PR flow. Describes itself as an open-source Claude Code alternative that connects arbitrary LLMs, edits code, runs commands, and validates changes. | Closest direct coding-agent competitor and the best local reference for agent-loop UX. | Study/benchmark; differentiate via source-traceable context, incremental freshness, and safety rather than generic autonomous coding. |
| [openJiuwen / agent-core](https://atomgit.com/openJiuwen/agent-core) | Apache-2.0; 3,621 stars, 843 forks, 2,160 commits. AI-agent development SDK covering runtime, tuning, and evolution. | Strong Chinese-native agent-framework comparison. | Architectural research; do not add a second framework without a measured need. |
| [openJiuwen / agent-studio](https://atomgit.com/openJiuwen/agent-studio) | Apache-2.0; 4,251 stars, 707 forks, 954 commits. Visual development/workflow environment for models, knowledge bases, and plugins. | Product/UX reference for visual orchestration and safe configuration. | Study UI/workflow ideas, not a dependency. |
| [openJiuwen / deepsearch](https://atomgit.com/openJiuwen/deepsearch) | 2,788 stars, 320 forks, 287 commits. Knowledge-enhanced deep-search/research framework with fragment-level citations and provenance reasoning. | Very close to PolyAgent's source-traceability and evidence-grounding goals. | High-priority comparison for cited context and research-agent behavior. |
| [openJiuwen / agent-memory](https://atomgit.com/openJiuwen/agent-memory) | Python, Apache-2.0; 712 stars, 63 forks, 116 commits. Provides extraction, storage, retrieval, and migration of long-term agent memory. | Direct reference for project/task-memory lifecycle. | Study data model; add branch/commit/worktree linkage, deletion controls, and stale/superseded facts. |
| [openJiuwen / agent-protocol](https://atomgit.com/openJiuwen/agent-protocol) | Apache-2.0; 3,043 stars, 325 forks. C++ SDK for agent communication, including MCP and A2A. | A Chinese ecosystem interoperability reference. | Later-phase only; least privilege and approval gates remain mandatory. |
| [CangjieMagic](https://atomgit.com/Cangjie-TPC/CangjieMagic) | Cangjie, MIT; 1,165 stars, 78 forks, 413 commits. Agent DSL/runtime with ReAct/tool loops, planning, RAG/vector search, MCP over stdio/SSE/HTTP, HITL, Ollama, and llama.cpp support. | Excellent architecture study showing how one ecosystem packages agents, tools, retrieval, multi-model routing, and human approval. | Reference only unless Cangjie becomes an explicit platform target. |
| [AI4SE / ISEA](https://atomgit.com/AI4SE/ISEA) | MIT engineering-focused SWE/code-agent development and benchmark framework; supports local/container execution, subagents, four-layer context compression, SWE-bench, and ProgramBench. | Direct evaluation and context-compression comparator. | Add to benchmark discovery; inspect methodology before relying on claims. |
| [GitCode MCP Server](https://atomgit.com/gitcode-ai/gitcode_mcp_server) | First-party GitCode/AtomGit MCP server; MIT; 37 stars, 22 forks, 16 commits. | Potential source connector if PolyAgent later needs AtomGit repository access. | Optional, read-only integration behind explicit installation/authorization. |
| [mcp-gitcode-readme](https://atomgit.com/master_hunter/mcp-gitcode-readme) | Apache-2.0; parses GitCode metadata/code structure, produces engineering/risk/README output, exposes MCP/HTTP/web console. | Direct comparator to repository understanding and generated `PROJECT_CONTEXT.md` features. | Study expected output and grounding; do not allow untrusted repo text to drive tools. |
| [DocKit](https://atomgit.com/weixin_38980638/DocKit) | TypeScript; Apache-2.0; 712 commits. Tauri (Rust + Vue) NoSQL desktop client with local/cloud providers, Ollama/LM Studio support, MCP, and explicit confirmation for destructive database operations. | Particularly relevant local-first desktop architecture and consent pattern. | Strong UX/security reference, but database-agent scope is different from code context. |
| [OpenHarmony-SIG / ohos_llama.cpp](https://atomgit.com/OpenHarmony-SIG/ohos_llama.cpp) | OpenHarmony adaptation of `llama.cpp`. | China-platform/local-runtime portability research. | Niche hardware/OS reference, not a universal desktop backend. |
| [Cangjie / CangjieCorpus](https://atomgit.com/Cangjie/CangjieCorpus) | Structured Cangjie language corpus intended as a RAG knowledge base for coding assistants. | Useful multilingual-code/RAG dataset and corpus-governance reference. | Benchmark only after confirming data license, provenance, and task fit. |

### Confirmed mirrors or secondary hosting: useful, but not canonical dependencies

| AtomGit project | Provenance finding | PolyAgent use |
|---|---|---|
| [Repomix](https://atomgit.com/GitHub_Trending/rep/repomix) | Explicit real-time GitHub mirror of `yamadashy/repomix`. | Essential static-bundle baseline; continue citing/using the upstream project. |
| [llama.cpp](https://atomgit.com/GitHub_Trending/ll/llama.cpp) | Explicit real-time GitHub mirror of `ggml-org/llama.cpp`. | Convenient China-hosted mirror; pin official upstream release and digest for any sidecar. |
| [Ollama](https://atomgit.com/GitHub_Trending/oll/ollama) | Explicit real-time GitHub mirror of `ollama/ollama`. | Use as optional local-endpoint/model-manager reference, not a bundled v1 dependency. |
| [Milvus](https://atomgit.com/GitHub_Trending/mi/milvus) | Explicit real-time GitHub mirror of `milvus-io/milvus`. | Remote/advanced retrieval reference; too heavy for the initial embedded desktop stack. |
| [Gitleaks](https://atomgit.com/GitHub_Trending/gi/gitleaks) | GitHub_Trending mirror of `gitleaks/gitleaks`. | Keep as a pre-export/pre-prompt scanning baseline; source rules/releases upstream. |
| [Tauri](https://atomgit.com/tauri-apps/tauri), [LightRAG](https://atomgit.com/HKUDS/LightRAG), [GraphRAG](https://atomgit.com/microsoft/graphrag), and [codebase-memory-mcp](https://atomgit.com/DeusData/codebase-memory-mcp) | Search result metadata matches well-known upstream projects; treat AtomGit pages as discovery/availability copies unless an upstream confirms a dual-host workflow. | Do not double-count these as China-native alternatives. |
| [UltraRAG](https://atomgit.com/OpenBMB/UltraRAG), [Nexent](https://atomgit.com/ModelEngine/nexent), [5ire](https://atomgit.com/Ironben/5ire), and [AntSK](https://atomgit.com/shuyu-labs/AntSK) | Pages/READMEs link to canonical GitHub projects, so classify as dual-hosted or secondary copies. | Useful product references for RAG/MCP/local desktop patterns; source dependency decisions upstream. |

### Platform observations that change the research plan

1. **The valuable AtomGit-specific ecosystem is not the mirror catalogue.** The most distinctive additions are AtomCode, the openJiuwen family, CangjieMagic, ISEA, the first-party GitCode MCP server, and repository-README tooling.
2. **OpenJiuwen is the strongest Chinese-agent comparison cluster.** It spans core runtime, visual studio, cited deep search, long-term memory, and protocol interoperability. It is the right ecosystem to compare against PolyAgent's proposed multi-agent/memory roadmap.
3. **CangjieMagic validates the need for separable capability adapters.** Its model/provider, RAG, tool-loop, MCP, and HITL building blocks reinforce PolyAgent's proposed architecture, but its Cangjie runtime is not a drop-in Tauri/Rust dependency.
4. **AtomGit mirrors are operationally useful, but never weaken supply-chain controls.** Pin immutable upstream versions and verify hashes/licenses; the hosting path does not establish code or model provenance.
5. **Exact-name result gaps are not absence evidence.** Public searches for Tauri, Qdrant, Code2Prompt, Gitingest, RAGFlow, FastGPT, TruffleHog, and Semgrep had inconsistent or empty result listings during the scan. This likely reflects indexing/search behavior, so it must not be interpreted as ecosystem absence.

### Recommended next research use

1. Benchmark PolyAgent's Phase-1 output against the AtomGit-mirrored [Repomix](https://atomgit.com/GitHub_Trending/rep/repomix) and the existing canonical GitHub baselines.
2. Add openJiuwen `deepsearch` and `agent-memory` to the architecture-comparison reading list; compare citation granularity, provenance, update semantics, and safety controls.
3. Study AtomCode and DocKit for approval, audit, local-first, and cross-platform UX patterns, without importing their broad execution privileges.
4. Treat the GitCode MCP Server as a later, optional AtomGit connector. It needs its own user consent, scoped repository access, and read-only default.
