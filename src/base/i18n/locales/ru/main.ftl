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
view-extensions = Расширения
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
command-workbench-show-extensions = Открыть расширения
command-workbench-quick-open = Поиск файлов, символов и команд
command-workbench-show-command-palette = Показать все команды
quick-open-commands-placeholder = Введите название команды
quick-open-no-results = Ничего не найдено
command-workbench-toggle-component-gallery = Показать или скрыть витрину компонентов

## Рабочая папка

window-title-workspace = { $name } — { -product-name }
command-workspace-open-folder = Открыть папку
command-workspace-open-folder-by-path = Открыть папку по пути
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
welcome-start-search = Поиск файлов и команд
welcome-start-open-folder = Открыть папку
welcome-start-panel = Нижняя панель
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
