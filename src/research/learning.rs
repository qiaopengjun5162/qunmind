#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LearningResourceCategory {
    LlmFoundations,
    ApiCalling,
    CodingAgents,
    AgentFoundations,
    AgentFrameworks,
    HermesExecution,
    WechatAgentSkills,
    MultimodalAgents,
    FormalMethods,
    LearningWorkflow,
    AiWeb3Bridge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LearningResourceFormat {
    Video,
    Course,
    Docs,
    Plan,
    Article,
    Prompt,
    Handbook,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LearningResource {
    pub title: &'static str,
    pub category: LearningResourceCategory,
    pub provider: &'static str,
    pub focus: &'static str,
    pub format: LearningResourceFormat,
    pub priority: u8,
    pub url: Option<&'static str>,
}

pub const LEARNING_RESOURCES: &[LearningResource] = &[
    LearningResource {
        title: "What is a Large Language Model?",
        category: LearningResourceCategory::LlmFoundations,
        provider: "General LLM intro",
        focus: "建立 LLM 最小必要直觉",
        format: LearningResourceFormat::Video,
        priority: 10,
        url: None,
    },
    LearningResource {
        title: "Hugging Face LLM Course Chapter 1",
        category: LearningResourceCategory::LlmFoundations,
        provider: "Hugging Face",
        focus: "系统理解 LLM 工作原理",
        format: LearningResourceFormat::Course,
        priority: 20,
        url: None,
    },
    LearningResource {
        title: "LLM API 调用入门",
        category: LearningResourceCategory::ApiCalling,
        provider: "General API intro",
        focus: "边看边跟着写 API 调用",
        format: LearningResourceFormat::Video,
        priority: 30,
        url: None,
    },
    LearningResource {
        title: "Anthropic: Building with the Claude API",
        category: LearningResourceCategory::ApiCalling,
        provider: "Anthropic",
        focus: "官方 Claude API 完整入门课程",
        format: LearningResourceFormat::Course,
        priority: 40,
        url: None,
    },
    LearningResource {
        title: "Z.ai API 开发者文档",
        category: LearningResourceCategory::ApiCalling,
        provider: "Z.ai",
        focus: "GLM MaaS API 入门、OpenAI 兼容接口和首个请求",
        format: LearningResourceFormat::Docs,
        priority: 50,
        url: None,
    },
    LearningResource {
        title: "Z.ai Coding Plan",
        category: LearningResourceCategory::ApiCalling,
        provider: "Z.ai",
        focus: "GLM 全系列模型调用能力",
        format: LearningResourceFormat::Plan,
        priority: 60,
        url: None,
    },
    LearningResource {
        title: "Claude Code 101",
        category: LearningResourceCategory::CodingAgents,
        provider: "Anthropic",
        focus: "AI coding 工具快速上手",
        format: LearningResourceFormat::Course,
        priority: 70,
        url: None,
    },
    LearningResource {
        title: "Agent Skills Engineering Workflow",
        category: LearningResourceCategory::CodingAgents,
        provider: "Addy Osmani",
        focus: "参考上下文工程、源码驱动决策、故障恢复和发布前验证的可复用工程流程；不直接安装第三方 hooks、命令或跨模型 CLI",
        format: LearningResourceFormat::Docs,
        priority: 75,
        url: Some("https://github.com/addyosmani/agent-skills"),
    },
    LearningResource {
        title: "AI Agent 入门",
        category: LearningResourceCategory::AgentFoundations,
        provider: "General agent intro",
        focus: "Agent 基础概念",
        format: LearningResourceFormat::Video,
        priority: 80,
        url: None,
    },
    LearningResource {
        title: "Microsoft AI Agents for Beginners",
        category: LearningResourceCategory::AgentFoundations,
        provider: "Microsoft",
        focus: "建立 agent 整体直觉，从概念到代码",
        format: LearningResourceFormat::Course,
        priority: 90,
        url: None,
    },
    LearningResource {
        title: "OpenAI Agents SDK Intro",
        category: LearningResourceCategory::AgentFrameworks,
        provider: "OpenAI",
        focus: "理解 agent 框架如何组织模型、工具与执行",
        format: LearningResourceFormat::Docs,
        priority: 100,
        url: None,
    },
    LearningResource {
        title: "LangGraph Overview",
        category: LearningResourceCategory::AgentFrameworks,
        provider: "LangChain",
        focus: "理解 agent 的组织方式",
        format: LearningResourceFormat::Docs,
        priority: 110,
        url: None,
    },
    LearningResource {
        title: "DojoAgents Architecture",
        category: LearningResourceCategory::AgentFrameworks,
        provider: "Alpha Dojo",
        focus: "学习 Agent Loop、Gateway adapter、记忆来源约束与 Skill 懒加载的分层边界，不引入投资执行能力",
        format: LearningResourceFormat::Docs,
        priority: 115,
        url: Some("https://github.com/Alpha-Dojo/DojoAgents"),
    },
    LearningResource {
        title: "Hermes Agent Docs",
        category: LearningResourceCategory::HermesExecution,
        provider: "Hermes",
        focus: "理解 agent、tool calling、skills、记忆和长期执行",
        format: LearningResourceFormat::Docs,
        priority: 120,
        url: None,
    },
    LearningResource {
        title: "Zread.ai 解读 OpenClaw / Hermes",
        category: LearningResourceCategory::HermesExecution,
        provider: "Zread.ai",
        focus: "理解 agent 进入执行层后的架构变化",
        format: LearningResourceFormat::Article,
        priority: 130,
        url: None,
    },
    LearningResource {
        title: "wx-cli 的 AI Agent Skill 来了！让 AI 直接帮你管微信私域",
        category: LearningResourceCategory::WechatAgentSkills,
        provider: "CC的AI轻百科",
        focus: "将 wx-cli 包装为 AI Agent Skill，让自然语言驱动微信私域数据分析和自动化任务",
        format: LearningResourceFormat::Article,
        priority: 135,
        url: Some("https://mp.weixin.qq.com/s/djeUolNR4bms8wGsvBTAZQ"),
    },
    LearningResource {
        title: "JoyAI-VL-Interaction",
        category: LearningResourceCategory::MultimodalAgents,
        provider: "JD Open Source",
        focus: "理解实时视频-语言交互、边看边说、说/停/委托决策和多模态 Agent 运行时",
        format: LearningResourceFormat::Docs,
        priority: 138,
        url: Some("https://github.com/jd-opensource/JoyAI-VL-Interaction"),
    },
    LearningResource {
        title: "Lean 4 中文文档",
        category: LearningResourceCategory::FormalMethods,
        provider: "Lean 中文社区",
        focus: "用中文建立 Lean 4 与定理证明器的基础使用直觉",
        format: LearningResourceFormat::Docs,
        priority: 139,
        url: Some("https://www.leanprover.cn/#lean-zh_1"),
    },
    LearningResource {
        title: "Lean4 通关互动小游戏",
        category: LearningResourceCategory::FormalMethods,
        provider: "Lean Game",
        focus: "通过交互式练习熟悉 Lean 4 的基本证明步骤与反馈循环",
        format: LearningResourceFormat::Course,
        priority: 139,
        url: Some("https://adam.math.hhu.de/#/"),
    },
    LearningResource {
        title: "Logical Foundations",
        category: LearningResourceCategory::FormalMethods,
        provider: "Software Foundations",
        focus: "系统学习命题逻辑、证明与 Coq / 形式化验证基础",
        format: LearningResourceFormat::Handbook,
        priority: 139,
        url: Some("https://softwarefoundations.cis.upenn.edu/lf-current/index.html"),
    },
    LearningResource {
        title: "AI × Web3 School Learning Agent 启动 Prompt",
        category: LearningResourceCategory::LearningWorkflow,
        provider: "AI x Web3 School",
        focus: "初始化个人学习计划、GitHub 学习仓库、每日打卡和 Handbook feedback 流程",
        format: LearningResourceFormat::Prompt,
        priority: 140,
        url: Some("https://aiweb3.school/learning-agent.zh.txt"),
    },
    LearningResource {
        title: "AI Content Knowledge Base",
        category: LearningResourceCategory::LearningWorkflow,
        provider: "mrbear1024",
        focus: "学习 raw / sources / products / wiki / staging 分层、来源追溯和 review-first 知识沉淀，不把 AI 草稿直接当作事实",
        format: LearningResourceFormat::Docs,
        priority: 145,
        url: Some("https://github.com/mrbear1024/ai-content-kb"),
    },
    LearningResource {
        title: "AI × Web3 School Handbook",
        category: LearningResourceCategory::AiWeb3Bridge,
        provider: "AI x Web3 School",
        focus: "AI 基础、Web3 基础、AI × Web3 Bridge 和前沿探索知识地图",
        format: LearningResourceFormat::Handbook,
        priority: 150,
        url: Some("https://aiweb3.school/zh/handbook/"),
    },
];

pub fn resources_by_category(category: LearningResourceCategory) -> Vec<&'static LearningResource> {
    LEARNING_RESOURCES
        .iter()
        .filter(|resource| resource.category == category)
        .collect()
}

pub fn foundation_path() -> Vec<&'static LearningResource> {
    LEARNING_RESOURCES
        .iter()
        .filter(|resource| {
            matches!(
                resource.category,
                LearningResourceCategory::LlmFoundations | LearningResourceCategory::ApiCalling
            )
        })
        .collect()
}

pub fn agent_path() -> Vec<&'static LearningResource> {
    LEARNING_RESOURCES
        .iter()
        .filter(|resource| {
            matches!(
                resource.category,
                LearningResourceCategory::CodingAgents
                    | LearningResourceCategory::AgentFoundations
                    | LearningResourceCategory::AgentFrameworks
                    | LearningResourceCategory::HermesExecution
                    | LearningResourceCategory::WechatAgentSkills
                    | LearningResourceCategory::MultimodalAgents
            )
        })
        .collect()
}

pub fn ai_web3_path() -> Vec<&'static LearningResource> {
    LEARNING_RESOURCES
        .iter()
        .filter(|resource| {
            matches!(
                resource.category,
                LearningResourceCategory::LlmFoundations
                    | LearningResourceCategory::AgentFoundations
                    | LearningResourceCategory::AgentFrameworks
                    | LearningResourceCategory::LearningWorkflow
                    | LearningResourceCategory::AiWeb3Bridge
            )
        })
        .collect()
}

pub fn find_resource(title: &str) -> Option<&'static LearningResource> {
    let normalized = normalize_title(title);
    LEARNING_RESOURCES
        .iter()
        .find(|resource| normalize_title(resource.title) == normalized)
}

fn normalize_title(title: &str) -> String {
    title
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resource(title: &str) -> &'static LearningResource {
        match find_resource(title) {
            Some(resource) => resource,
            None => panic!("resource: {title}"),
        }
    }

    #[test]
    fn covers_all_requested_learning_resources() {
        let titles = LEARNING_RESOURCES
            .iter()
            .map(|resource| resource.title)
            .collect::<Vec<_>>();

        for title in [
            "What is a Large Language Model?",
            "Hugging Face LLM Course Chapter 1",
            "LLM API 调用入门",
            "Anthropic: Building with the Claude API",
            "Z.ai API 开发者文档",
            "Z.ai Coding Plan",
            "Claude Code 101",
            "Agent Skills Engineering Workflow",
            "AI Agent 入门",
            "Microsoft AI Agents for Beginners",
            "OpenAI Agents SDK Intro",
            "LangGraph Overview",
            "Hermes Agent Docs",
            "Zread.ai 解读 OpenClaw / Hermes",
            "wx-cli 的 AI Agent Skill 来了！让 AI 直接帮你管微信私域",
            "JoyAI-VL-Interaction",
            "Lean 4 中文文档",
            "Lean4 通关互动小游戏",
            "Logical Foundations",
            "AI × Web3 School Learning Agent 启动 Prompt",
            "AI × Web3 School Handbook",
        ] {
            assert!(titles.contains(&title), "missing {title}");
        }
    }

    #[test]
    fn covers_core_learning_categories() {
        for category in [
            LearningResourceCategory::LlmFoundations,
            LearningResourceCategory::ApiCalling,
            LearningResourceCategory::CodingAgents,
            LearningResourceCategory::AgentFoundations,
            LearningResourceCategory::AgentFrameworks,
            LearningResourceCategory::HermesExecution,
            LearningResourceCategory::WechatAgentSkills,
            LearningResourceCategory::MultimodalAgents,
            LearningResourceCategory::FormalMethods,
            LearningResourceCategory::LearningWorkflow,
            LearningResourceCategory::AiWeb3Bridge,
        ] {
            assert!(!resources_by_category(category).is_empty());
        }
    }

    #[test]
    fn foundation_path_starts_from_llm_basics() {
        let path = foundation_path();

        assert_eq!(path[0].title, "What is a Large Language Model?");
        assert_eq!(path[1].title, "Hugging Face LLM Course Chapter 1");
        assert!(
            path.iter()
                .any(|resource| resource.title == "Anthropic: Building with the Claude API")
        );
        assert!(path.iter().all(|resource| resource.priority < 70));
    }

    #[test]
    fn agent_path_keeps_frameworks_and_execution_layer_together() {
        let titles = agent_path()
            .into_iter()
            .map(|resource| resource.title)
            .collect::<Vec<_>>();

        assert!(titles.contains(&"Microsoft AI Agents for Beginners"));
        assert!(titles.contains(&"Agent Skills Engineering Workflow"));
        assert!(titles.contains(&"OpenAI Agents SDK Intro"));
        assert!(titles.contains(&"LangGraph Overview"));
        assert!(titles.contains(&"Hermes Agent Docs"));
        assert!(titles.contains(&"Zread.ai 解读 OpenClaw / Hermes"));
        assert!(titles.contains(&"wx-cli 的 AI Agent Skill 来了！让 AI 直接帮你管微信私域"));
        assert!(titles.contains(&"JoyAI-VL-Interaction"));
        assert!(!titles.contains(&"Hugging Face LLM Course Chapter 1"));
    }

    #[test]
    fn ai_web3_path_includes_learning_workflow_and_handbook() {
        let titles = ai_web3_path()
            .into_iter()
            .map(|resource| resource.title)
            .collect::<Vec<_>>();

        assert!(titles.contains(&"What is a Large Language Model?"));
        assert!(titles.contains(&"Microsoft AI Agents for Beginners"));
        assert!(titles.contains(&"AI × Web3 School Learning Agent 启动 Prompt"));
        assert!(titles.contains(&"AI × Web3 School Handbook"));
        assert!(!titles.contains(&"Z.ai Coding Plan"));
    }

    #[test]
    fn finds_resources_with_normalized_titles() {
        let claude = resource("anthropic building with the claude api");
        let agent_skills = resource("agent skills engineering workflow");
        let zread = resource("Zread AI OpenClaw Hermes");
        let wx_cli_skill = resource("wx-cli 的 AI Agent Skill 来了！让 AI 直接帮你管微信私域");
        let joyai = resource("joy ai vl interaction");
        let lean = resource("Lean 4 中文文档");
        let logical_foundations = resource("Logical Foundations");
        let handbook = resource("AI Web3 School Handbook");

        assert_eq!(claude.provider, "Anthropic");
        assert_eq!(claude.category, LearningResourceCategory::ApiCalling);
        assert_eq!(
            agent_skills.category,
            LearningResourceCategory::CodingAgents
        );
        assert_eq!(
            agent_skills.url,
            Some("https://github.com/addyosmani/agent-skills")
        );
        assert_eq!(zread.category, LearningResourceCategory::HermesExecution);
        assert_eq!(
            wx_cli_skill.category,
            LearningResourceCategory::WechatAgentSkills
        );
        assert_eq!(joyai.category, LearningResourceCategory::MultimodalAgents);
        assert_eq!(lean.category, LearningResourceCategory::FormalMethods);
        assert_eq!(
            logical_foundations.url,
            Some("https://softwarefoundations.cis.upenn.edu/lf-current/index.html")
        );
        assert_eq!(handbook.category, LearningResourceCategory::AiWeb3Bridge);
        assert_eq!(handbook.url, Some("https://aiweb3.school/zh/handbook/"));
    }
}
