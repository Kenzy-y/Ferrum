# 0011. Статус-бар, toolchain и иконки файлов по макету

**Дата:** 2026-11-08. **Статус:** принято владельцем («сделай статус-бар и всё, чего нет по макету»).

## Контекст

У владельца rust-analyzer не запускался: в toolchain `stable` не было компонента, прокси rustup сразу завершался. Ferrum этого никак не показал. В `editor.svg` в статус-баре есть «✓ rust-analyzer», язык «Rust» и версия toolchain «1.82.0». В `welcome.svg` есть «Rust toolchain ready» и «Rust 1.82.0». У `.rs` иконка «R», справа в хлебных крошках «Rust / 2021 edition».

## Решение

- **Состояние сервера** выводится в статус-бар для каждого запущенного сервера папки:
  - готов — ✓;
  - запуск или индексация — значок обновления и сообщение;
  - сбой — ✗, а в подсказке причина.
- **Причина сбоя:** `lsp` хранит последние строки stderr сервера, и `ServerStatus::Failed` их получает. `LspAdapter::failure_hint` переводит известный вывод в id сообщения i18n (`ServerStatus::Failed.hint`). Так `lang_rust` распознаёт «Unknown binary rust-analyzer» и подсказывает `rustup component add rust-analyzer`.
- **Toolchain и детали файла** — новая точка вклада языкового пакета, трейт `language::Toolchain`:
  - `version(root)` выдаёт версию toolchain, у Rust это `rustc --version` в папке проекта, с учётом `rust-toolchain.toml`;
  - `file_detail(file)` выдаёт короткую деталь файла, у Rust это `edition` из ближайшего `Cargo.toml`.
  Всё запускается в фоне через `ProcessRunner` и `Fs`. Ядро ничего не знает о Rust.
- **Иконка файлов языка** объявляется в пакете (`LanguagePack::with_file_icon`: буква и токен цвета). Shell регистрирует её в `FileIcons`.

## Последствия

- В `language` добавилась зависимость от `fs`, в `lang_rust` — от `fs` и `toml` (оба уже есть в workspace).
- Колокольчик уведомлений, разделение редактора, меню «…» у вкладок, ветка git и кнопка запуска ждут своих этапов (ROADMAP).
