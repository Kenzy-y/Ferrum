# Строки оболочки приложения.
# Название продукта подставляет крейт `i18n` как терм `-product-name`.

app-title = { -product-name }

## Команды

command-app-quit = Выйти из { -product-name }

## Рабочая область

app-wordmark = { -product-wordmark }
menu-file = Файл
menu-edit = Правка
menu-view = Вид
menu-go = Переход
titlebar-search-placeholder = Поиск команд и настроек
titlebar-open-folder = Открыть папку
titlebar-run-project = Запустить проект
view-explorer = Проводник
view-search = Поиск
view-source-control = Система контроля версий
view-run-and-debug = Запуск и отладка
view-extensions = Плагины
panel-terminal = Терминал
panel-problems = Проблемы
panel-output = Вывод
panel-debug-console = Консоль отладки
status-no-folder = Папка не открыта
command-workbench-toggle-sidebar = Показать или скрыть боковую панель
command-workbench-toggle-panel = Показать или скрыть нижнюю панель
command-workbench-show-explorer = Открыть проводник
command-workbench-show-search = Открыть поиск
command-workbench-show-source-control = Открыть систему контроля версий
command-workbench-show-run-and-debug = Открыть запуск и отладку
command-workbench-show-extensions = Открыть плагины
command-workbench-quick-open = Поиск файлов, символов и команд
command-workbench-show-command-palette = Показать все команды
quick-open-commands-placeholder = Введите название команды
quick-open-no-results = Ничего не найдено
command-workbench-toggle-component-gallery = Показать или скрыть витрину компонентов

## Рабочая папка

window-title-workspace = { $name } — { -product-name }
command-workspace-open-folder = Открыть папку
command-workspace-open-folder-by-path = Открыть папку по пути
command-preferences-open-settings = Открыть настройки
command-preferences-open-settings-file = Открыть файл настроек (TOML)
command-preferences-open-keybindings = Открыть сочетания клавиш
command-preferences-open-keymap = Открыть файл сочетаний клавиш (TOML)
command-workbench-zoom-in = Увеличить масштаб
command-workbench-zoom-out = Уменьшить масштаб
command-workbench-zoom-reset = Сбросить масштаб
config-settings-header =
    Настройки. Применяются при сохранении файла.
    Пример:
    {"["}editor]
    tab_size = 4
config-keymap-header =
    Сочетания клавиш. Применяются при сохранении файла.
    Пример:
    {"["}editor]
    "ctrl-d" = "editor.addCursorBelow"
path-prompt-title = Открыть папку
path-prompt-placeholder = Путь к папке
path-prompt-open = Открыть
path-prompt-cancel = Отмена
open-folder-picker-unavailable = Системный диалог выбора папки недоступен ({ $error }). Введите путь к папке вручную.
open-folder-not-found = Папка { $path } не существует.
open-folder-not-a-folder = { $path } — это не папка.
open-folder-failed = Не удалось открыть { $path }: { $error }
explorer-empty-title = Папка не открыта
explorer-empty-text = Откройте папку, чтобы увидеть файлы проекта.
explorer-files = Файлы
explorer-outline = Структура
outline-empty = В активном редакторе нет символов
file-tree-cycle = Ссылка на одну из родительских папок; не раскрывается
file-tree-failed = Не удалось прочитать папку

## Экран приветствия

welcome-tab = Добро пожаловать
welcome-title = Немного фокуса. Много Rust.
welcome-subtitle = Ваш следующий отличный проект начинается здесь.
welcome-new-project = Новый проект Rust
welcome-clone = Клонировать репозиторий
welcome-later = Появится в одной из следующих версий
welcome-recent = Недавние проекты
welcome-recent-empty-title = С чистого листа
welcome-recent-empty-text = Здесь появятся недавно открытые проекты.
welcome-get-started = Начало работы
welcome-start-palette = Палитра команд
welcome-start-find-file = Найти файл
welcome-start-settings = Открыть настройки
welcome-start-terminal = Показать терминал
welcome-workflow-title = Создано для вашей работы
welcome-workflow-text = Rust-analyzer, Cargo и Git. Готовы, когда готовы вы.
welcome-docs-link = Читать документацию

## Витрина компонентов (временная, этап 1)

gallery-title = Витрина компонентов
gallery-note = Временный экран для сравнения компонентов с макетами. В колонке «Живой» наводите, нажимайте и переходите по Tab.
gallery-section-colors = Цветовые токены
gallery-section-syntax = Токены синтаксиса
gallery-section-typography = Типографика
gallery-section-buttons = Кнопки
gallery-section-badges = Бейджи
gallery-section-tabs = Вкладки
gallery-section-panels = Панели и заголовки секций
gallery-section-lists = Списки и activity bar
gallery-section-inputs = Поля ввода
gallery-section-icons = Иконки
gallery-state-normal = Обычное
gallery-state-hover = Наведение
gallery-state-pressed = Нажатие
gallery-state-focused = Фокус
gallery-state-disabled = Неактивное
gallery-state-live = Живой
gallery-variant-primary = Основная
gallery-variant-secondary = Вторичная
gallery-variant-ghost = Прозрачная
gallery-variant-icon = Кнопка-иконка
gallery-variant-medium = Основная, крупная
gallery-type-display = Немного фокуса. Много Rust.
gallery-type-heading = Языковая поддержка Rust
gallery-type-title = Настройте рабочее место под себя
gallery-type-body = Ваш следующий отличный проект начинается здесь.
gallery-type-secondary = Откройте папку, чтобы увидеть файлы проекта.
gallery-type-caption = Здесь появятся недавно открытые проекты.
gallery-type-mono = let config = config::load(&cli.config)?;
gallery-sample-run-project = Запустить проект
gallery-sample-open-folder = Открыть папку
gallery-sample-new-project = Новый проект Rust
gallery-sample-explorer = Проводник
gallery-sample-files = Файлы
gallery-sample-outline = Структура
gallery-sample-terminal = Терминал
gallery-sample-problems = Проблемы
gallery-sample-output = Вывод
gallery-sample-installed = Установленные
gallery-sample-marketplace = Каталог
gallery-sample-search-placeholder = Поиск файлов, символов и команд
gallery-sample-field-placeholder = Поиск расширений
gallery-sample-commit-placeholder = Сообщение коммита
gallery-sample-card-title = С чистого листа
gallery-sample-card-text = Здесь появятся недавно открытые проекты.
gallery-sample-row-selected = Выбранная
gallery-sample-row-ignored = Игнорируемая
gallery-sample-row-nested = Вложенная
gallery-sample-tab-active = Активная
gallery-sample-tab-dirty = Не сохранена

## Editor

command-editor-move-left = Курсор влево
command-editor-move-right = Курсор вправо
command-editor-move-up = Курсор вверх
command-editor-move-down = Курсор вниз
command-editor-move-word-left = Курсор к предыдущему слову
command-editor-move-word-right = Курсор к следующему слову
command-editor-move-to-line-start = Курсор в начало строки
command-editor-move-to-line-end = Курсор в конец строки
command-editor-move-to-document-start = Курсор в начало файла
command-editor-move-to-document-end = Курсор в конец файла
command-editor-page-up = Страница вверх
command-editor-page-down = Страница вниз
command-editor-select-left = Выделить влево
command-editor-select-right = Выделить вправо
command-editor-select-up = Выделить вверх
command-editor-select-down = Выделить вниз
command-editor-select-word-left = Выделить до предыдущего слова
command-editor-select-word-right = Выделить до следующего слова
command-editor-select-to-line-start = Выделить до начала строки
command-editor-select-to-line-end = Выделить до конца строки
command-editor-select-to-document-start = Выделить до начала файла
command-editor-select-to-document-end = Выделить до конца файла
command-editor-select-page-up = Выделить страницу вверх
command-editor-select-page-down = Выделить страницу вниз
command-editor-add-cursor-above = Добавить курсор выше
command-editor-add-cursor-below = Добавить курсор ниже
command-editor-single-cursor = Оставить только основной курсор
command-editor-select-all = Выделить всё
command-editor-undo = Отменить
command-editor-redo = Повторить
command-editor-copy = Копировать
command-editor-cut = Вырезать
command-editor-paste = Вставить
command-editor-delete-left = Удалить слева
command-editor-delete-right = Удалить справа
command-editor-delete-word-left = Удалить предыдущее слово
command-editor-delete-word-right = Удалить следующее слово
command-editor-newline = Новая строка
command-editor-indent = Увеличить отступ
command-editor-outdent = Уменьшить отступ
command-editor-save = Сохранить
command-editor-reload-from-disk = Загрузить с диска
command-editor-keep-local-changes = Оставить мои изменения
command-editor-line-endings-lf = Привести окончания строк к LF
command-editor-line-endings-crlf = Привести окончания строк к CRLF
command-editor-trigger-suggest = Показать подсказки дополнения
command-editor-show-hover = Показать сведения о символе
command-editor-go-to-definition = Перейти к определению
command-workbench-close-tab = Закрыть вкладку
command-workbench-next-tab = Следующая вкладка
command-workbench-previous-tab = Предыдущая вкладка
command-workbench-close-window = Закрыть окно
command-workbench-focus-outline = Перейти к структуре
command-workbench-toggle-outline = Свернуть или развернуть структуру
command-workbench-open-menu-bar = Открыть меню
command-workbench-toggle-terminal = Показать или скрыть терминал
command-workbench-toggle-maximized-panel = Развернуть или восстановить панель
command-terminal-new = Новый терминал
command-terminal-split = Разделить терминал
command-terminal-kill = Закрыть терминал
command-terminal-select-shell = Выбрать оболочку
command-terminal-focus-next = Следующий терминал
command-terminal-focus-previous = Предыдущий терминал
command-terminal-copy = Копировать из терминала
command-terminal-paste = Вставить в терминал
command-terminal-select-all = Выделить всё в терминале
command-terminal-clear = Очистить терминал
command-terminal-scroll-page-up = Прокрутить терминал на страницу вверх
command-terminal-scroll-page-down = Прокрутить терминал на страницу вниз
command-terminal-scroll-to-top = Прокрутить терминал в начало
command-terminal-scroll-to-bottom = Прокрутить терминал в конец
editor-close-unsaved = Сохранить изменения в { $name }?
editor-close-unsaved-detail = Если не сохранить, изменения пропадут.
editor-untitled = Без имени
editor-close-save = Сохранить
editor-close-discard = Не сохранять
editor-close-cancel = Отмена
editor-overwrite = { $name } изменён на диске. Перезаписать его вашей версией?
editor-overwrite-detail = Изменения на диске пропадут.
editor-overwrite-confirm = Перезаписать
window-close-unsaved = { $count ->
    [one] Сохранить изменения в { $count } файле?
   *[other] Сохранить изменения в { $count } файлах?
}
window-close-save-all = Сохранить всё
editor-disk-changed = Файл изменён на диске.
editor-disk-deleted = Файл удалён на диске.
editor-disk-reload = Загрузить с диска
editor-disk-keep = Оставить мои
editor-disk-save-again = Сохранить заново
editor-disk-close = Закрыть
editor-open-binary = { $name } — двоичный файл, его нельзя открыть как текст.
editor-open-not-utf8 = { $name } не в кодировке UTF-8. Другие кодировки пока не поддерживаются.
editor-open-failed = Не удалось открыть { $name }: { $error }
editor-save-failed = Не удалось сохранить { $name }: { $error }
status-position = Стр { $line }, стлб { $column }
status-selections = Выделений: { $count }
status-indent-spaces = Пробелы: { $size }
status-indent-tabs = Размер табуляции: { $size }
status-encoding = UTF-8
status-line-ending-lf = LF
status-line-ending-crlf = CRLF
status-line-ending-mixed = Смешанные
status-errors = { $count ->
    [one] { $count } ошибка
    [few] { $count } ошибки
   *[many] { $count } ошибок
}
status-warnings = { $count ->
    [one] { $count } предупреждение
    [few] { $count } предупреждения
   *[many] { $count } предупреждений
}
status-toolchain-ready = Toolchain { $language } готов
status-server-starting = { $name }: запуск
status-server-indexing = { $name }: { $detail }
status-server-failed = { $name } остановлен.
status-server-not-found = { $name } не найден: { $program }
lang-rust-analyzer-not-installed = Для этого toolchain не установлен rust-analyzer. Выполните: rustup component add rust-analyzer
lang-rust-edition = редакция { $value }
problems-empty = В рабочей области проблем не найдено.
problems-position = Стр { $line }, стлб { $column }
terminal-no-shell = На этом компьютере не найдено ни одной оболочки.
terminal-finding-shells = Поиск оболочек…
terminal-start-failed = Не удалось запустить { $shell }: { $error }
terminal-hint-toggle = Показать терминал
terminal-hint-palette = Палитра команд
command-explorer-new-file = Новый файл
new-file-title = Новый файл
new-file-placeholder = Имя файла, например src/main.rs
new-file-create = Создать
new-file-error-empty = Введите имя файла.
new-file-error-outside = Файл должен быть внутри выбранной папки.
new-file-error-exists = Файл или папка с таким именем уже есть.
new-file-error-failed = Не удалось создать файл: { $error }

## Search view
search-placeholder = Поиск
search-match-case = С учётом регистра
search-whole-word = Слово целиком
search-regex = Регулярное выражение
search-replace = Заменить
search-include = Включить файлы
search-include-placeholder = например, src/**/*.rs, !target
search-searching = Идёт поиск…
search-no-results = Ничего не найдено
search-no-folder = Откройте папку, чтобы искать по её файлам.
search-summary = { $results ->
    [one] { $results } совпадение
    [few] { $results } совпадения
   *[many] { $results } совпадений
} в { $files ->
    [one] { $files } файле
   *[other] { $files } файлах
}
search-summary-limited = Первые { $results } совпадений в { $files ->
    [one] { $files } файле
   *[other] { $files } файлах
} (поиск остановлен на пределе)
search-invalid-pattern = Неверное регулярное выражение: { $error }
search-invalid-glob = Неверный шаблон файлов: { $error }
search-in = Поиск в { $name }

## Run and Debug view
debug-no-configuration = Нет конфигурации запуска
debug-variables = Переменные
debug-variables-empty = Программа не приостановлена.
debug-watch = Отслеживание
debug-watch-empty = Нет выражений для отслеживания.
debug-call-stack = Стек вызовов
debug-call-stack-empty = Нет сеанса отладки.
debug-breakpoints = Точки останова
debug-breakpoints-empty = Нет точек останова.
debug-all-exceptions = Все исключения
debug-uncaught-exceptions = Необработанные исключения

## Extensions view
extensions-search-placeholder = Поиск плагинов
extensions-installed = Установленные
extensions-marketplace = Маркетплейс
extensions-marketplace-later = Маркетплейс появится вместе с системой плагинов.
extensions-none-found = Подходящих плагинов нет.
extensions-enabled = Включён
extensions-disabled = Отключён
extensions-workspace = Рабочая область
extensions-enabled-count = { $count ->
    [one] Включён { $count } плагин
    [few] Включено { $count } плагина
   *[many] Включено { $count } плагинов
}
extensions-up-to-date = Все плагины обновлены
extensions-close-page = Закрыть
extensions-disable = Отключить
extensions-enable = Включить
extensions-uninstall = Удалить
extensions-install = Установить
extensions-installed-enabled = Установлен и включён
extensions-installed-disabled = Установлен, отключён
extensions-enabled-workspace = Включён в этой рабочей области
extensions-disabled-workspace = Отключён в этой рабочей области
extensions-overview = Обзор
extensions-contributions = Вклады
extensions-no-contributions = Этот плагин пока ничего не добавляет.
extensions-more-title = Настройте рабочую область под себя

## The sample extension (until the plugin system)
sample-extension-name = Пример плагина
sample-extension-publisher = { -product-name }
sample-extension-summary = Так будут выглядеть плагины
sample-extension-description = Показывает, как будут выглядеть плагины. Пока ничего не делает.
sample-extension-overview-title = { -product-name } под ваши задачи
sample-extension-overview-text = Плагины работают в песочнице WebAssembly и добавляют возможности, не замедляя редактор.
sample-extension-commands = Команды
sample-extension-commands-detail = Действия со своими сочетаниями клавиш и пунктами палитры.
sample-extension-views = Панели и виды
sample-extension-views-detail = Виды боковой панели, вкладки и элементы статус-бара.
sample-extension-languages = Языки и темы
sample-extension-languages-detail = Языковые пакеты, цветовые темы и иконки.
sample-extension-code-comment = # extension.toml — любой плагин начинается с манифеста.
sample-extension-side-title = Что внутри плагина
sample-extension-manifest = Манифест
sample-extension-manifest-detail = extension.toml с id, версией и API
sample-extension-sandbox = Песочница
sample-extension-sandbox-detail = Пределы времени и памяти для каждого плагина
sample-extension-permissions = Права
sample-extension-permissions-detail = Вы подтверждаете их при установке
sample-extension-contribution-commands = Команды
sample-extension-contribution-settings = Настройки
sample-extension-contribution-views = Виды
sample-extension-contribution-none = Нет

# Страница настроек.
settings-page-title = Настройки
settings-page-search = Поиск настроек
settings-page-tab-settings = Настройки
settings-page-tab-keybindings = Сочетания клавиш
settings-page-layer-user = Пользователь
settings-page-layer-workspace = Рабочая область
settings-page-no-workspace = Откройте папку, чтобы менять её настройки
settings-page-open-file = Открыть settings.toml
settings-page-reset = Сбросить
settings-page-modified = Изменено
settings-page-no-results = Настройки не найдены
settings-page-toml-only = Изменить в settings.toml
settings-page-all = Все настройки
settings-page-on = Вкл.
settings-page-off = Выкл.
settings-page-invalid = Это значение не подходит для настройки
keybindings-search = Поиск сочетаний
keybindings-command = Команда
keybindings-keys = Клавиши
keybindings-context = Контекст
keybindings-change = Изменить
keybindings-add = Добавить
keybindings-remove = Удалить
keybindings-record = Нажмите клавиши… (Escape — отмена)
keybindings-user = свои
keybindings-open-file = Открыть keymap.toml
settings-group-appearance = Внешний вид
settings-group-editor = Редактор
settings-group-files = Файлы
settings-group-keyboard = Клавиатура
settings-group-localization = Язык
settings-group-lsp = Языковые серверы
settings-group-project = Проект
settings-group-rust_analyzer = rust-analyzer
settings-group-search = Поиск
settings-group-syntax = Синтаксис
settings-group-terminal = Терминал

# Поиск в редакторе.
command-find-open = Найти
command-find-replace = Заменить
command-find-next = Найти далее
command-find-previous = Найти назад
find-placeholder = Найти
find-replace-placeholder = Заменить
find-match-case = С учётом регистра
find-whole-word = Слово целиком
find-regex = Регулярное выражение
find-previous = Предыдущее совпадение (Shift Enter)
find-next = Следующее совпадение (Enter)
find-close = Закрыть (Escape)
find-replace = Заменить (Enter)
find-replace-all = Заменить все (Ctrl Alt Enter)
find-toggle-replace = Показать замену
find-no-results = Нет результатов
find-invalid = Неверный шаблон
find-count = { $current } из { $total }
find-count-limited = { $current } из { $total }+

search-replace-placeholder = Заменить
search-replace-all = Заменить все (Enter)
search-replace-collapse = Скрыть замену
