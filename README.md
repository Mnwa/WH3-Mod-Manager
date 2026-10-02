# WH3 Mod Manager · Rust

Нативный менеджер модов Total War: Warhammer III (Steam) на Rust и GPUI Kit.
Первый этап переноса [Shazbot/WH3-Mod-Manager](https://github.com/Shazbot/WH3-Mod-Manager).
Полный список реализованного и оставшихся отличий — [docs/MIGRATION.md](docs/MIGRATION.md).

## Запуск

Нужен актуальный **stable Rust**. Версия компилятора не закреплена.
Windows: Visual Studio Build Tools с C++ и Windows SDK, CMake. macOS: Command Line Tools.

```sh
cargo run -p wh3-mod-manager --release --locked
# Демонстрация большой библиотеки без установки игры:
cargo run -p wh3-mod-manager --release --locked -- --demo=100000
```

Выберите папку с `Warhammer3.exe`. Менеджер сканирует `data` и соседнюю Steam Workshop
папку `1142710`; дополнительные папки можно добавить вручную. Включайте моды флажками,
выделяйте строку для изменения порядка. «Включить» / «Отключить» под фильтрами меняют все найденные моды. Поиск работает по названию, pack, Workshop ID,
импортированному автору, тегам и категориям. Сортировка списка не меняет порядок запуска.

`Ctrl/Cmd+F` — поиск; `Ctrl/Cmd+S` — сохранить; `Alt+↑/↓` — сдвинуть выделенный мод;
`Esc` — закрыть отчёт. При закрытии изменённая библиотека сохраняется автоматически;
при ошибке записи окно остаётся открытым с сообщением.

Кнопка «Играть» доступна в Windows. Создаётся отдельный `wh3_rust_mods.txt`;
`used_mods.txt` оригинала не изменяется. Запуск с реальной игрой требует ручной проверки
на Windows; автоматические тесты проверяют состав списка и аргументов.

## Перенос метаданных оригинала

В оригинальном менеджере сначала сохраните настройки. Его `config.json` находится в
каталоге пользовательских данных Electron либо рядом с exe (portable-вариант).

В Rust-менеджере:

1. «Мета из оригинала…» → выбрать оригинальный `config.json` → сохранить `wh3-metadata.json`.
2. «Импорт метаданных» → выбрать экспортированный файл (либо сразу `config.json`).
3. Проверить отчёт об отсутствующих модах, порядок и включённые моды; сохранить библиотеку.

Экспортёр также доступен отдельно, без GUI, Steam и Node.js:

```sh
cargo run -p wh3-core --bin wh3-meta --locked -- export config.json wh3-metadata.json
# Из Windows-архива:
wh3-meta.exe export config.json wh3-metadata.json
```

Исходный config не изменяется. Экспорт включает сохранённые названия, авторов,
категории, теги, Workshop ID, зависимости и пресеты, если эти поля есть в исходнике.
Данные, которые оригинал не сохранил, не выдумываются и не скачиваются.

Внутреннее хранилище — бинарный `library.whmm` на **rkyv**, не JSON.
Формат, резервная копия и восстановление описаны в [docs/STORAGE.md](docs/STORAGE.md).

## Проверки и Windows EXE

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo bench -p wh3-core --bench catalog --locked
```

На macOS тест `visual` использует настоящий Metal-рендерер, создаёт снимки в
`target/visual` и проверяет виртуализацию на 1 000 / 10 000 / 100 000 модов.
На остальных платформах этот конкретный тест явно пропускается.

GitHub Actions проверяет ядро на Linux, весь workspace на Windows/macOS и собирает
Windows x64 release. Артефакт `WH3-Mod-Manager-windows-x64` содержит GUI exe,
`wh3-meta.exe`, ZIP и SHA-256 суммы. Исполняемые файлы не подписаны.

## Структура

- `crates/core`: каталог, Steam-пути, pack-индексы, пресеты, метаданные, бинарное хранилище, запуск.
- `crates/desktop`: GPUI, компактные компоненты интерфейса, фоновые задачи и виртуальные списки.
- `docs/PERFORMANCE.md`: измерения и границы проверенного.

За основу взаимодействий и форматов взят Shazbot (MIT); подходы к GPUI и rkyv —
локальный проект `cr-chat-desktop`. Уведомление об авторских правах оригинала сохранено в LICENSE.
