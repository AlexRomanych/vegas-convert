use anyhow::{Context, Result};
use calamine::{DataType, Reader, Xlsx, open_workbook};
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ModelConstructProcedure {
    pub code_1c: String,          // Первичный ключ
    pub name: String,             // Название процедуры
    pub text: Option<String>,     // Текст (может быть пустым)
    pub text_vba: Option<String>, // Адаптированный под VBA (может быть пустым)
    pub object_code_1c: Option<String>,
    pub object_name: Option<String>,
}

// __ Создаем "ленивое" хранилище
static KEYS_MATRIX: OnceLock<HashMap<&str, &str>> = OnceLock::new();
fn get_keys_matrix() -> &'static HashMap<&'static str, &'static str> {
    KEYS_MATRIX.get_or_init(|| {
        HashMap::from([
            ("If", "If"),
            ("Then", "Then"),
            ("ElseIf", "ElseIf"),
            ("End", "End"),
            ("Else", "Else"),
            ("Not", "Not"),
            ("Or", "Or"),
            ("And", "And"),
            ("Round", "Round"),
            ("Fix", "Fix"),
            ("=", "="),
            (">", ">"),
            ("<", "<"),
            ("-", "-"),
            ("+", "+"),
            ("*", "*"),
            ("/", "/"),
            ("'", "'"),
            ("(", "("),
            (")", ")"),
        ])
    })
}

pub async fn run(
    tx: &mut Transaction<'_, Postgres>,
    path: &str,
    start_row: usize,
    code_1c_length: &usize,
) -> Result<()> {
    // println!("code_1c_length: {}", code_1c_length);

    // __ Очищает данные и сбрасывает счетчики ID (SERIAL) в начальное состояние
    sqlx::query("TRUNCATE TABLE models RESTART IDENTITY CASCADE")
        .execute(&mut **tx)
        .await?;

    let mut workbook: Xlsx<_> = open_workbook(path)
        .with_context(|| format!("Не удалось открыть файл процедур: {}", path))?;

    // Предполагаем, что данные на первом листе
    let range = workbook
        .worksheet_range_at(0)
        .context("Лист в файле процедур не найден")??;

    let mut count = 0;

    for row in range.rows().skip(start_row) {
        // Пропускаем заголовок Excel
        // Маппинг колонок (индексы 0, 1, 2... должны соответствовать порядку в Excel)

        let raw_text = row.get(2);

        let procedure = ModelConstructProcedure {
            // Используем .map вместо .and_then(|c| Some(c...)), так короче
            code_1c: {
                let raw = row.get(0).map(|c| c.to_string()).unwrap_or_default();
                if raw.is_empty() {
                    raw // возвращает пустую String
                } else {
                    format!("{:0>width$}", raw, width = code_1c_length) // возвращает отформатированную String
                }
            },

            name: row
                .get(1)
                .map(|c| c.to_string())
                .unwrap_or_else(|| "Без названия".to_string()),

            text: raw_text.map(|c| c.to_string()),
            text_vba: raw_text.map(|c| convert_to_vba(c.to_string())),
            // text_vba: Some("123".to_string()),
            object_code_1c: {
                let raw = row.get(3).map(|c| c.to_string()).unwrap_or_default();
                if raw.is_empty() {
                    None // Если пусто, возвращаем Option::None (в БД будет NULL)
                } else {
                    // Форматируем и оборачиваем в Some
                    Some(format!("{:0>width$}", raw, width = code_1c_length))
                }
            },

            object_name: row.get(4).map(|c| c.to_string()),
            // code_1c: row.get(0).and_then(|c| Some(c.to_string())).unwrap_or_default(),
            // name: row.get(1).and_then(|c| Some(c.to_string())).unwrap_or_else(|| "Без названия".into()),
            // text: row.get(2).map(|c| c.to_string()),
            // object_code_1c: row.get(3).map(|c| c.to_string()),
            // object_name: row.get(4).map(|c| c.to_string()),
        };

        if procedure.code_1c.is_empty() {
            continue;
        }

        // Выполняем вставку с обновлением при конфликте (Upsert)
        sqlx::query(
            r#"
                INSERT INTO model_construct_procedures (
                    code_1c,
                    name,
                    text,
                    text_vba,
                    object_code_1c,
                    object_name,
                    updated_at,
                    created_at

                )
                VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())
                ON CONFLICT (code_1c) DO UPDATE SET
                    name = EXCLUDED.name,
                    text = EXCLUDED.text,
                    text_vba = EXCLUDED.text_vba,
                    object_code_1c = EXCLUDED.object_code_1c,
                    object_name = EXCLUDED.object_name,
                    updated_at = NOW()
           "#,
        )
        .bind(&procedure.code_1c)
        .bind(&procedure.name)
        .bind(&procedure.text)
        .bind(&procedure.text_vba)
        .bind(&procedure.object_code_1c)
        .bind(&procedure.object_name)
        .execute(&mut **tx)
        .await?;

        count += 1;
    }

    println!("✅ Процедуры: импортировано {} строк", count);
    Ok(())
}

fn convert_to_vba(excel_text: String) -> String {
    let lines: Vec<&str> = excel_text
        .lines() // Создает итератор по строкам
        .map(|s| s.trim()) // Убирает лишние пробелы по краям каждой строки
        // .filter(|s| !s.is_empty()) // Удаляет пустые строки (если они не нужны)
        .collect(); // Собирает всё в вектор

    let mut vba_vector: Vec<String> = Vec::new(); // __ Вектор строк процедуры
    let mut vba_variables: HashMap<String, String> = HashMap::new(); // __ HashMap переменных процедуры

    for (index, line) in lines.iter().enumerate() {
        if line.is_empty() {
            let next_line = lines.get(index + 1);
            if let Some(l) = next_line {
                if l.is_empty() {
                    continue;
                } else {
                    push_vba_line(line, &mut vba_vector, &mut vba_variables);
                    // vba_vector.push(line);
                }
            }
        } else {
            push_vba_line(line, &mut vba_vector, &mut vba_variables);
            // vba_vector.push(line);
        }
    }

    vba_vector.join("\n")
}

fn push_vba_line(
    vba_line: &str,
    vba_lines_vector: &mut Vec<String>,
    vba_procedure_vars: &mut HashMap<String, String>,
) {
    let mut vba_line_str = vba_line.to_string();

    vba_line_str = vba_line_str.replace("//", "' "); // __ переделываем комментарии
    vba_line_str = vba_line_str.replace(";", ""); // __ убираем ";"

    // ' избавляемся от табуляции
    //     .procedureParsedText = Replace(.procedureParsedText, Chr(TAB_CHAR), "")

    // __ переделываем условные операторы, порядок имеет значение
    vba_line_str = vba_line_str.replace("ИначеЕсли", "ElseIf");
    vba_line_str = vba_line_str.replace("Иначе", "Else");
    vba_line_str = vba_line_str.replace("КонецЕсли", "End If");
    vba_line_str = vba_line_str.replace("Если", "If");

    vba_line_str = vba_line_str.replace("Тогда", "Then");
    vba_line_str = vba_line_str.replace("тогда", "Then");

    vba_line_str = vba_line_str.replace(" не ", " Not ");
    vba_line_str = vba_line_str.replace(" или ", " Or ");
    vba_line_str = vba_line_str.replace(" и ", " And ");
    vba_line_str = vba_line_str.replace("Окр", "Round");
    vba_line_str = vba_line_str.replace("Цел", "Fix");

    // __ отделяем пробелами ключевые слова
    vba_line_str = vba_line_str.replace("=", " = ");
    vba_line_str = vba_line_str.replace(">", " > ");
    vba_line_str = vba_line_str.replace("<", " < ");
    vba_line_str = vba_line_str.replace("-", " - ");
    vba_line_str = vba_line_str.replace("+", " + ");
    vba_line_str = vba_line_str.replace("*", " * ");
    vba_line_str = vba_line_str.replace("/", " / ");
    // vba_line_str = vba_line_str.replace("'", " ' ");
    vba_line_str = vba_line_str.replace("(", " ( ");
    vba_line_str = vba_line_str.replace(")", " ) ");

    // __ Удаляем "  "
    while vba_line_str.contains("  ") {
        vba_line_str = vba_line_str.replace("  ", " ");
    }

    vba_line_str = translate_line(vba_line_str, vba_procedure_vars);

    vba_lines_vector.push(vba_line_str);
}

fn translate_line(russian_line: String, vars: &mut HashMap<String, String>) -> String {
    let dictionary = get_keys_matrix();

    // __ Используем collect, чтобы не было проблем с временем жизни ссылки на russian_line
    let words: Vec<&str> = russian_line.split_whitespace().collect();

    for word in words {
        if dictionary.get(word).is_none() {
            // __ Сразу создаем String нужной емкости (примерно в 1.5 раза больше исходного слова)
            let mut translit_result = String::with_capacity(word.len() * 2);

            for c in word.chars() {
                let trans_c = match c {
                    ' ' => "_",
                    'а' => "a",
                    'б' => "b",
                    'в' => "v",
                    'г' => "g",
                    'д' => "d",
                    'е' => "e",
                    'ё' => "e",
                    'ж' => "zh",
                    'з' => "z",
                    'и' => "i",
                    'й' => "j",
                    'к' => "k",
                    'л' => "l",
                    'м' => "m",
                    'н' => "n",
                    'о' => "o",
                    'п' => "p",
                    'р' => "r",
                    'с' => "s",
                    'т' => "t",
                    'у' => "u",
                    'ф' => "f",
                    'х' => "h",
                    'ц' => "c",
                    'ч' => "ch",
                    'ш' => "sh",
                    'щ' => "shch",
                    'ъ' | 'ь' => "",
                    'ы' => "y",
                    'э' => "e",
                    'ю' => "yu",
                    'я' => "ya",
                    _ => {
                        // Для спецсимволов используем временный буфер,
                        // так как нам нужно превратить char в строку
                        translit_result.push(c);
                        continue; // Переходим к следующему символу
                    }
                };

                translit_result.push_str(trans_c);
            }

            // __ Записываем переменную в мапу переменных
            vars.insert(word.to_string(), translit_result);
            // Клонируем строку, чтобы использовать её и как ключ, и как значение
            // vars.insert(translit_result.clone(), translit_result);
        }
    }

    let mut result = russian_line;
    for (rus_name, vba_name) in vars.iter() {
        result = result.replace(rus_name, vba_name);
        // println!("Заменяем '{}' на '{}'", rus_name, vba_name);
    }
    
    result
}

/*
Основные моменты реализации:
Типы данных: * CODE_1C из Laravel стал String, так как это первичный ключ в виде строки.
Поля nullable() в миграции стали Option<String> в Rust.
Upsert (ON CONFLICT): Поскольку 1С часто выгружает данные повторно, мы используем DO UPDATE. Это обновит существующие записи, если code_1c уже есть в базе.
Производительность: Функция принимает &mut Transaction. Это позволяет вызвать импорт процедур в рамках одного большого процесса в main.rs. Если один файл упадет — откатится всё.
Безопасность: Использование unwrap_or_default() и map() предотвращает "панику" программы, если в Excel встретится пустая ячейка в середине данных.
 */
