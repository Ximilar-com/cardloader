//! The few texts the backend shows the user itself: the OS notification when a run ends. Everything else the UI
//! translates (`src/i18n`); error messages stay English.

use crate::model::{Session, SessionStatus};

struct Texts {
    done: &'static str,
    /// The card count: singular, plural (the same in languages without plurals).
    cards: (&'static str, &'static str),
    to_review: &'static str,
    failed: &'static str,
    stopped: &'static str,
    an_error: &'static str,
}

fn texts(language: &str) -> Texts {
    match language {
        "ja" => Texts {
            done: "完了",
            cards: ("{n}枚", "{n}枚"),
            to_review: "要確認 {n}枚",
            failed: "失敗 {n}枚",
            stopped: "停止",
            an_error: "エラー",
        },
        "es" => Texts {
            done: "Listo",
            cards: ("{n} carta", "{n} cartas"),
            to_review: "{n} por revisar",
            failed: "{n} con error",
            stopped: "Detenida",
            an_error: "un error",
        },
        "ko" => Texts {
            done: "완료",
            cards: ("카드 {n}장", "카드 {n}장"),
            to_review: "검토 필요 {n}장",
            failed: "실패 {n}장",
            stopped: "중지됨",
            an_error: "오류",
        },
        "fr" => Texts {
            done: "Terminé",
            cards: ("{n} carte", "{n} cartes"),
            to_review: "{n} à vérifier",
            failed: "{n} en échec",
            stopped: "Arrêtée",
            an_error: "une erreur",
        },
        "pt" => Texts {
            done: "Concluído",
            cards: ("{n} carta", "{n} cartas"),
            to_review: "{n} para revisar",
            failed: "{n} com falha",
            stopped: "Interrompida",
            an_error: "um erro",
        },
        "de" => Texts {
            done: "Fertig",
            cards: ("{n} Karte", "{n} Karten"),
            to_review: "{n} zu prüfen",
            failed: "{n} fehlgeschlagen",
            stopped: "Gestoppt",
            an_error: "ein Fehler",
        },
        "it" => Texts {
            done: "Fatto",
            cards: ("{n} carta", "{n} carte"),
            to_review: "{n} da rivedere",
            failed: "{n} non riuscite",
            stopped: "Interrotta",
            an_error: "un errore",
        },
        _ => Texts {
            done: "Done",
            cards: ("{n} card", "{n} cards"),
            to_review: "{n} to review",
            failed: "{n} failed",
            stopped: "Stopped",
            an_error: "an error",
        },
    }
}

/// "Done: 412 cards, $1284.50 · 23 to review · 2 failed", or why the run failed, in the app's language.
pub fn finished_text(session: &Session, language: &str) -> String {
    let t = texts(language);
    // French writes a space before the colon.
    let colon = if language == "fr" { " : " } else { ": " };
    let c = &session.counts;
    if session.status == SessionStatus::Failed {
        return format!(
            "{}{colon}{}",
            t.stopped,
            session.error.as_deref().unwrap_or(t.an_error)
        );
    }
    // French counts 0 and 1 as singular.
    let singular = c.done == 1 || (language == "fr" && c.done == 0);
    let cards = if singular { t.cards.0 } else { t.cards.1 };
    let mut text = format!(
        "{}{colon}{}",
        t.done,
        cards.replace("{n}", &c.done.to_string())
    );
    if c.total_price > 0.0 {
        text += &format!(", ${:.2}", c.total_price);
    }
    if c.to_review > 0 {
        text += " · ";
        text += &t.to_review.replace("{n}", &c.to_review.to_string());
    }
    if c.errors > 0 {
        text += " · ";
        text += &t.failed.replace("{n}", &c.errors.to_string());
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{IdMode, SessionCounts, SessionOptions, SourceKind};

    fn session(status: SessionStatus, counts: SessionCounts, error: Option<&str>) -> Session {
        Session {
            id: "s1".into(),
            name: "Demo".into(),
            source_kind: SourceKind::Folder,
            source_ref: String::new(),
            id_mode: IdMode::Auto,
            options: SessionOptions::default(),
            status,
            error: error.map(Into::into),
            created_at: String::new(),
            started_at: None,
            finished_at: None,
            remote_session_id: None,
            sync_error: None,
            ebay_error: None,
            ebay_checked_at: None,
            counts,
        }
    }

    #[test]
    fn notification_speaks_the_app_language() {
        let done = session(
            SessionStatus::Completed,
            SessionCounts {
                done: 412,
                total_price: 1284.5,
                to_review: 23,
                errors: 2,
                ..Default::default()
            },
            None,
        );
        assert_eq!(
            finished_text(&done, "en"),
            "Done: 412 cards, $1284.50 · 23 to review · 2 failed"
        );
        assert_eq!(
            finished_text(&done, "de"),
            "Fertig: 412 Karten, $1284.50 · 23 zu prüfen · 2 fehlgeschlagen"
        );
        assert_eq!(
            finished_text(&done, "ja"),
            "完了: 412枚, $1284.50 · 要確認 23枚 · 失敗 2枚"
        );
        // Unknown codes read as English.
        assert_eq!(finished_text(&done, "xx"), finished_text(&done, "en"));

        let one = session(
            SessionStatus::Completed,
            SessionCounts {
                done: 1,
                ..Default::default()
            },
            None,
        );
        assert_eq!(finished_text(&one, "en"), "Done: 1 card");
        assert_eq!(finished_text(&one, "fr"), "Terminé : 1 carte");

        let failed = session(SessionStatus::Failed, SessionCounts::default(), None);
        assert_eq!(finished_text(&failed, "es"), "Detenida: un error");
    }
}
