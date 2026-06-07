#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LearningResourceCategory {
    LlmFoundations,
    ApiCalling,
    CodingAgents,
    AgentFoundations,
    AgentFrameworks,
    HermesExecution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LearningResourceFormat {
    Video,
    Course,
    Docs,
    Plan,
    Article,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LearningResource {
    pub title: &'static str,
    pub category: LearningResourceCategory,
    pub provider: &'static str,
    pub focus: &'static str,
    pub format: LearningResourceFormat,
    pub priority: u8,
}

pub const LEARNING_RESOURCES: &[LearningResource] = &[
    LearningResource {
        title: "What is a Large Language Model?",
        category: LearningResourceCategory::LlmFoundations,
        provider: "General LLM intro",
        focus: "建立 LLM 最小必要直觉",
        format: LearningResourceFormat::Video,
        priority: 10,
    },
    LearningResource {
        title: "Hugging Face LLM Course Chapter 1",
        category: LearningResourceCategory::LlmFoundations,
        provider: "Hugging Face",
        focus: "系统理解 LLM 工作原理",
        format: LearningResourceFormat::Course,
        priority: 20,
    },
    LearningResource {
        title: "LLM API 调用入门",
        category: LearningResourceCategory::ApiCalling,
        provider: "General API intro",
        focus: "边看边跟着写 API 调用",
        format: LearningResourceFormat::Video,
        priority: 30,
    },
    LearningResource {
        title: "Anthropic: Building with the Claude API",
        category: LearningResourceCategory::ApiCalling,
        provider: "Anthropic",
        focus: "官方 Claude API 完整入门课程",
        format: LearningResourceFormat::Course,
        priority: 40,
    },
    LearningResource {
        title: "Z.ai API 开发者文档",
        category: LearningResourceCategory::ApiCalling,
        provider: "Z.ai",
        focus: "GLM MaaS API 入门、OpenAI 兼容接口和首个请求",
        format: LearningResourceFormat::Docs,
        priority: 50,
    },
    LearningResource {
        title: "Z.ai Coding Plan",
        category: LearningResourceCategory::ApiCalling,
        provider: "Z.ai",
        focus: "GLM 全系列模型调用能力",
        format: LearningResourceFormat::Plan,
        priority: 60,
    },
    LearningResource {
        title: "Claude Code 101",
        category: LearningResourceCategory::CodingAgents,
        provider: "Anthropic",
        focus: "AI coding 工具快速上手",
        format: LearningResourceFormat::Course,
        priority: 70,
    },
    LearningResource {
        title: "AI Agent 入门",
        category: LearningResourceCategory::AgentFoundations,
        provider: "General agent intro",
        focus: "Agent 基础概念",
        format: LearningResourceFormat::Video,
        priority: 80,
    },
    LearningResource {
        title: "Microsoft AI Agents for Beginners",
        category: LearningResourceCategory::AgentFoundations,
        provider: "Microsoft",
        focus: "建立 agent 整体直觉，从概念到代码",
        format: LearningResourceFormat::Course,
        priority: 90,
    },
    LearningResource {
        title: "OpenAI Agents SDK Intro",
        category: LearningResourceCategory::AgentFrameworks,
        provider: "OpenAI",
        focus: "理解 agent 框架如何组织模型、工具与执行",
        format: LearningResourceFormat::Docs,
        priority: 100,
    },
    LearningResource {
        title: "LangGraph Overview",
        category: LearningResourceCategory::AgentFrameworks,
        provider: "LangChain",
        focus: "理解 agent 的组织方式",
        format: LearningResourceFormat::Docs,
        priority: 110,
    },
    LearningResource {
        title: "Hermes Agent Docs",
        category: LearningResourceCategory::HermesExecution,
        provider: "Hermes",
        focus: "理解 agent、tool calling、skills、记忆和长期执行",
        format: LearningResourceFormat::Docs,
        priority: 120,
    },
    LearningResource {
        title: "Zread.ai 解读 OpenClaw / Hermes",
        category: LearningResourceCategory::HermesExecution,
        provider: "Zread.ai",
        focus: "理解 agent 进入执行层后的架构变化",
        format: LearningResourceFormat::Article,
        priority: 130,
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
            "AI Agent 入门",
            "Microsoft AI Agents for Beginners",
            "OpenAI Agents SDK Intro",
            "LangGraph Overview",
            "Hermes Agent Docs",
            "Zread.ai 解读 OpenClaw / Hermes",
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
        assert!(titles.contains(&"OpenAI Agents SDK Intro"));
        assert!(titles.contains(&"LangGraph Overview"));
        assert!(titles.contains(&"Hermes Agent Docs"));
        assert!(titles.contains(&"Zread.ai 解读 OpenClaw / Hermes"));
        assert!(!titles.contains(&"Hugging Face LLM Course Chapter 1"));
    }

    #[test]
    fn finds_resources_with_normalized_titles() {
        let claude = find_resource("anthropic building with the claude api").expect("claude api");
        let zread = find_resource("Zread AI OpenClaw Hermes").expect("zread hermes");

        assert_eq!(claude.provider, "Anthropic");
        assert_eq!(claude.category, LearningResourceCategory::ApiCalling);
        assert_eq!(zread.category, LearningResourceCategory::HermesExecution);
    }
}
