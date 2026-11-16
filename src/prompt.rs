//! Personas and layered system prompts.
//!
//! A request's system prompt is assembled from three layers, from broadest to
//! most specific:
//!
//! 1. the global prompt (settings, applies to every chat);
//! 2. the persona prompt (role and tone, chosen per chat);
//! 3. the chat prompt (instructions for one conversation).
//!
//! Every layer may use `{{variables}}`, expanded at send time so dates and
//! model names are always current.

use serde::{Deserialize, Serialize};

/// Persona used by new chats and as a fallback for missing ids.
pub const DEFAULT_PERSONA: &str = "builtin:assistant";

#[derive(Clone, Serialize, Deserialize)]
pub struct Persona {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub prompt: String,
    /// Seeded by the app rather than created by the user.
    #[serde(default)]
    pub builtin: bool,
}

pub fn builtins() -> Vec<Persona> {
    vec![
        Persona {
            id: DEFAULT_PERSONA.to_owned(),
            name: "Ассистент".to_owned(),
            description: "Спокойный универсал".to_owned(),
            prompt: "Ты — Muninn, приватный ассистент, который работает прямо на устройстве пользователя. \
Отвечай по делу, дружелюбно и на языке пользователя. Не выдумывай факты: если не уверен, скажи об этом. \
Используй markdown, когда он помогает читать ответ."
                .to_owned(),
            builtin: true,
        },
        Persona {
            id: "builtin:coder".to_owned(),
            name: "Программист".to_owned(),
            description: "Старший инженер, код без воды".to_owned(),
            prompt: "Ты — старший инженер-программист. Сначала разберись в задаче и контексте, затем дай рабочее \
решение. Код — в блоках с указанием языка, без псевдокода, если можно написать настоящий. Кратко объясняй \
ключевые решения и подводные камни. Не добавляй лишних зависимостей и не переписывай то, о чём не просили."
                .to_owned(),
            builtin: true,
        },
        Persona {
            id: "builtin:editor".to_owned(),
            name: "Редактор".to_owned(),
            description: "Ясность, ритм, живой язык".to_owned(),
            prompt: "Ты — редактор русского текста. Сохраняй смысл и голос автора, улучшай ясность, ритм и точность. \
Убирай канцелярит, штампы и повторы. Сначала покажи готовый текст, затем коротко перечисли, что изменил. \
Не добавляй фактов от себя."
                .to_owned(),
            builtin: true,
        },
        Persona {
            id: "builtin:tutor".to_owned(),
            name: "Наставник".to_owned(),
            description: "Объясняет по шагам".to_owned(),
            prompt: "Ты — терпеливый наставник. Объясняй по шагам, от простого к сложному, с примерами. \
Проверяй понимание вопросами. Если пользователь хочет научиться, веди его к ответу, а не выдавай готовое. \
Честно признавай, когда тема выходит за пределы твоих знаний."
                .to_owned(),
            builtin: true,
        },
        Persona {
            id: "builtin:translator".to_owned(),
            name: "Переводчик".to_owned(),
            description: "Естественный перевод".to_owned(),
            prompt: "Ты — профессиональный переводчик. Переводи естественно, сохраняя смысл, тон и терминологию. \
По умолчанию переводи на русский, если не указано иное. Имена, код и команды не переводи. Если термин \
неоднозначен, предложи вариант и объясни выбор."
                .to_owned(),
            builtin: true,
        },
        Persona {
            id: "builtin:analyst".to_owned(),
            name: "Аналитик".to_owned(),
            description: "Структура, риски, факты".to_owned(),
            prompt: "Ты — аналитик. Структурируй ответ: вывод, аргументы, риски, что проверить. Отделяй факты от \
предположений и явно называй неопределённость. Не подгоняй вывод под ожидания: если данных мало, скажи, каких \
именно не хватает."
                .to_owned(),
            builtin: true,
        },
    ]
}

/// Re-add built-in personas that the user deleted, without touching the rest.
pub fn restore(list: &mut Vec<Persona>) -> usize {
    let mut added = 0;
    for persona in builtins() {
        if !list.iter().any(|existing| existing.id == persona.id) {
            list.push(persona);
            added += 1;
        }
    }
    added
}

pub struct Vars<'a> {
    pub model: &'a str,
    pub chat: &'a str,
    pub persona: &'a str,
}

/// Join the non-empty layers and expand variables.
pub fn compose(global: &str, persona: Option<&str>, chat: &str, vars: &Vars<'_>) -> Option<String> {
    let mut sections = Vec::new();
    for layer in [global, persona.unwrap_or(""), chat] {
        let layer = layer.trim();
        if !layer.is_empty() {
            sections.push(layer.to_owned());
        }
    }

    if sections.is_empty() {
        return None;
    }

    Some(substitute(&sections.join("\n\n"), &local_now(), vars))
}

fn local_now() -> jiff::Zoned {
    let zone = crate::platform::timezone_id()
        .and_then(|id| jiff::tz::TimeZone::get(&id).ok())
        .unwrap_or_else(jiff::tz::TimeZone::system);
    jiff::Timestamp::now().to_zoned(zone)
}

fn substitute(text: &str, now: &jiff::Zoned, vars: &Vars<'_>) -> String {
    if !text.contains("{{") {
        return text.to_owned();
    }

    let date = now.strftime("%d.%m.%Y").to_string();
    let time = now.strftime("%H:%M").to_string();
    let weekday = weekday_ru(now.weekday());
    let zone = now.time_zone().iana_name().unwrap_or("локальное время");

    let datetime = format!("{date}, {time}");
    let values = [
        ("{{date}}", date.as_str()),
        ("{{time}}", time.as_str()),
        ("{{weekday}}", weekday),
        ("{{datetime}}", datetime.as_str()),
        ("{{tz}}", zone),
        ("{{model}}", vars.model),
        ("{{chat}}", vars.chat),
        ("{{persona}}", vars.persona),
        ("{{app}}", "Muninn"),
    ];

    let mut out = text.to_owned();
    for (key, value) in values {
        out = out.replace(key, value);
    }
    out
}

fn weekday_ru(weekday: jiff::civil::Weekday) -> &'static str {
    use jiff::civil::Weekday;
    match weekday {
        Weekday::Monday => "понедельник",
        Weekday::Tuesday => "вторник",
        Weekday::Wednesday => "среда",
        Weekday::Thursday => "четверг",
        Weekday::Friday => "пятница",
        Weekday::Saturday => "суббота",
        Weekday::Sunday => "воскресенье",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars() -> Vars<'static> {
        Vars {
            model: "test/model",
            chat: "Тестовый чат",
            persona: "Ассистент",
        }
    }

    #[test]
    fn empty_layers_produce_no_prompt() {
        assert!(compose("  ", None, "", &vars()).is_none());
    }

    #[test]
    fn layers_keep_order_and_skip_blank() {
        let prompt = compose("global", Some("persona"), "chat", &vars()).unwrap();
        assert_eq!(prompt, "global\n\npersona\n\nchat");

        let prompt = compose("", Some("  "), "chat", &vars()).unwrap();
        assert_eq!(prompt, "chat");
    }

    #[test]
    fn variables_expand() {
        let prompt = compose(
            "model={{model}} chat={{chat}} persona={{persona}} app={{app}}",
            None,
            "",
            &vars(),
        )
        .unwrap();
        assert_eq!(
            prompt,
            "model=test/model chat=Тестовый чат persona=Ассистент app=Muninn"
        );
    }

    #[test]
    fn date_variables_have_expected_shape() {
        let prompt = compose("{{date}} {{time}}", None, "", &vars()).unwrap();
        let mut parts = prompt.split(' ');
        let date = parts.next().unwrap();
        let time = parts.next().unwrap();
        assert_eq!(date.len(), 10);
        assert_eq!(time.len(), 5);
        assert!(date.contains('.'));
        assert!(time.contains(':'));
    }

    #[test]
    fn restore_adds_only_missing_builtins() {
        let mut list = builtins();
        assert_eq!(restore(&mut list), 0);
        list.retain(|persona| persona.id != "builtin:coder");
        assert_eq!(restore(&mut list), 1);
        assert!(list.iter().any(|persona| persona.id == "builtin:coder"));
    }
}
