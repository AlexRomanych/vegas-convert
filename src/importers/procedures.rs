use anyhow::{Context, Result};
use calamine::{DataType, Reader, Xlsx, open_workbook};
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};
use std::collections::HashMap;
use std::sync::OnceLock;

use crate::structures::procedure::{ModelConstructProcedure};

// #[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
// pub struct ModelConstructProcedure {
//     pub code_1c: String,          // Первичный ключ
//     pub name: String,             // Название процедуры
//     pub text: Option<String>,     // Текст (может быть пустым)
//     pub text_vba: Option<String>, // Адаптированный под VBA (может быть пустым)
//     pub object_code_1c: Option<String>,
//     pub object_name: Option<String>,
// }

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
        let procedure_name = row
            .get(1)
            .map(|c| c.to_string())
            .unwrap_or_else(|| "Без названия".to_string());

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

            text: raw_text.map(|c| c.to_string()),
            text_vba: raw_text.map(|c| convert_to_vba(c.to_string(), &procedure_name)),

            name: procedure_name,

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

        // __ Создаем строку запроса динамически
        let query_str = format!(
            r#"
                INSERT INTO {} (
                code_1c, name, text, text_vba, object_code_1c, object_name, updated_at, created_at
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
            ModelConstructProcedure::PROCEDURES_TABLE_NAME
        );

        // __ Выполняем вставку с обновлением при конфликте
        sqlx::query(&query_str)
            .bind(&procedure.code_1c)
            .bind(&procedure.name)
            .bind(&procedure.text)
            .bind(&procedure.text_vba)
            .bind(&procedure.object_code_1c)
            .bind(&procedure.object_name)
            .execute(&mut **tx)
            .await?;

        // sqlx::query(
        //     r#"
        //         INSERT INTO model_construct_procedures (
        //             code_1c,
        //             name,
        //             text,
        //             text_vba,
        //             object_code_1c,
        //             object_name,
        //             updated_at,
        //             created_at
        //
        //         )
        //         VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())
        //         ON CONFLICT (code_1c) DO UPDATE SET
        //             name = EXCLUDED.name,
        //             text = EXCLUDED.text,
        //             text_vba = EXCLUDED.text_vba,
        //             object_code_1c = EXCLUDED.object_code_1c,
        //             object_name = EXCLUDED.object_name,
        //             updated_at = NOW()
        //    "#,
        // )
        // .bind(&procedure.code_1c)
        // .bind(&procedure.name)
        // .bind(&procedure.text)
        // .bind(&procedure.text_vba)
        // .bind(&procedure.object_code_1c)
        // .bind(&procedure.object_name)
        // .execute(&mut **tx)
        // .await?;

        count += 1;
    }

    println!("✅ Процедуры: импортировано {} строк", count);
    Ok(())
}

fn convert_to_vba(excel_text: String, procedure_name: &String) -> String {
    let lines: Vec<&str> = excel_text
        .lines() // Создает итератор по строкам
        .map(|s| s.trim()) // Убирает лишние пробелы по краям каждой строки
        // .filter(|s| !s.is_empty()) // Удаляет пустые строки (если они не нужны)
        .collect(); // Собирает всё в вектор

    let mut vba_vector: Vec<String> = Vec::new(); // __ Вектор строк процедуры
    let mut vba_variables: HashMap<String, String> = HashMap::new(); // __ HashMap переменных процедуры

    // __ Определяем название процедуры
    let mut proc_name = procedure_name.clone();
    push_vba_line(proc_name.as_str(), &mut vba_vector, &mut vba_variables);
    proc_name = vba_vector.pop().unwrap_or("no_name".to_string());

    proc_name = proc_name
        .replace("  ", " ")
        .replace(" ", "_")
        .replace("*", "_")
        .replace("?", "_")
        .replace("/", "_")
        .replace("\\", "_")
        .replace(",", "_")
        .replace("(", "_")
        .replace(")", "_")
        .replace("+", "_")
        .replace("-", "_");

    // __ Очищаем массив переменных после формирования названия процедуры
    vba_variables.clear();

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

    // __ Собираем в конечную кучу
    let mut result_vector: Vec<String> = Vec::new();

    const FUNCTION_PREFIX: &str = "f_";

    // __ Формируем строку присвоения результата
    let mut assign_str = String::from("    ".to_owned() + FUNCTION_PREFIX);
    assign_str.push_str(proc_name.as_str());
    assign_str.push_str(" = result");

    // __ Формируем и отправляем заголовок функции
    let mut res_func_name = String::from("Function ".to_owned() + FUNCTION_PREFIX);
    res_func_name.push_str(proc_name.as_str());
    res_func_name.push_str("() as double");
    result_vector.push(res_func_name);

    const VARS_PER_STRING: u8 = 5; // __ Количество переменных в строке
    const INIT_STR: &str = "    Dim ";

    let mut vars_count: u8 = 0;
    let mut var = INIT_STR.to_string();

    for (_, lat_var) in vba_variables.iter() {
        if !lat_var.contains('.') && !lat_var.contains('[') && !lat_var.contains(']') {
            var.push_str(lat_var);
            var.push_str(" as double, ");

            vars_count += 1;

            if vars_count >= VARS_PER_STRING {
                var.pop();
                var.pop();
                result_vector.push(var.clone());
                var = INIT_STR.to_string();
                vars_count = 0;
            }
        }
    }

    if var.len() != INIT_STR.len() {
        var.pop();
        var.pop();
        result_vector.push(var.clone());
    }

    result_vector.push("    Dim result as double".to_string());
    result_vector.push("".to_string());

    let mut indent_level: i8 = 0;
    let mut keep_indent = false;
    let mut result_line = String::new();

    for line in vba_vector {
        // let normalized = &line;
        let normalized = line.to_lowercase();
        keep_indent = false;

        // __ Порядок важен
        // __ 1. Сначала проверяем закрытие (End If)
        if normalized.contains("end if") {
            indent_level -= 1;
            keep_indent = false;
        }
        // __ 2. Затем проверяем Else / ElseIf (они не меняют уровень, но сбрасывают keep_indent)
        else if normalized.contains("else") {
            keep_indent = true;
        }
        // __ 3. И только потом открытие нового блока
        else if normalized.starts_with("if ") && normalized.contains(" then") {
            indent_level += 1;
            keep_indent = true;
        }

        if keep_indent {
            result_line = get_indent_string_by_level(&(indent_level - 1)).to_string();
        } else {
            result_line = get_indent_string_by_level(&indent_level).to_string();
        }

        result_line.push_str(&line);
        result_vector.push(result_line);
    }

    // __ Отправляем строку присвоения результата
    result_vector.push("".to_string());
    result_vector.push(assign_str);

    result_vector.push("End function".to_string());

    result_vector.join("\n")
    // vba_vector.join("\n")
}

fn get_indent_string_by_level(indent_level: &i8) -> &str {
    if *indent_level == 1 {
        return "        ";
    } else if *indent_level == 2 {
        return "            ";
    } else if *indent_level == 3 {
        return "                ";
    } else if *indent_level == 4 {
        return "                    ";
    }

    "    "
}

/// __ Отправляем в вектор строк строку, попутно конвертируя ее в
/// __ транслитерацию и выделяем и собираем все переменные
fn push_vba_line(
    vba_line: &str,
    vba_lines_vector: &mut Vec<String>,
    vba_procedure_vars: &mut HashMap<String, String>,
) {
    let mut vba_line_str = vba_line.to_string();

    vba_line_str = vba_line_str.replace("//", "' "); // __ переделываем комментарии
    vba_line_str = vba_line_str.replace(";", ""); // __ убираем ";"

    // __ избавляемся от табуляции
    vba_line_str = vba_line_str.replace("/t", "");
    vba_line_str = vba_line_str.replace('\u{9}', "");

    // __ убираем неразрывные пробелы (код 160 - A0)
    vba_line_str = vba_line_str.replace('\u{A0}', " ");

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
    vba_line_str = vba_line_str.replace("ОКР", "Round");
    vba_line_str = vba_line_str.replace("Цел", "Fix");
    vba_line_str = vba_line_str.replace("ЦЕЛ", "Fix");

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
    vba_line_str = vba_line_str.replace(",", ", ");

    // __ Переводим в транслитерацию + выделяем все переменные
    vba_line_str = translate_line(vba_line_str, vba_procedure_vars);

    // __ Удаляем "  "
    while vba_line_str.contains("  ") {
        vba_line_str = vba_line_str.replace("  ", " ");
    }

    // __ возвращаем нормальное отображение скобок после отделения от переменных
    while vba_line_str.contains("( ") {
        vba_line_str = vba_line_str.replace("( ", "(");
    }
    while vba_line_str.contains(" )") {
        vba_line_str = vba_line_str.replace(" )", ")");
    }

    // __ возвращаем нормальное отображение больше-меньше-равно
    while vba_line_str.contains("> =") {
        vba_line_str = vba_line_str.replace("> =", ">=");
    }
    while vba_line_str.contains("< =") {
        vba_line_str = vba_line_str.replace("< =", "<=");
    }

    while vba_line_str.contains(" , ") {
        vba_line_str = vba_line_str.replace(" , ", ", ");
    }

    vba_line_str = vba_line_str.replace("Round (", "Round(");
    vba_line_str = vba_line_str.replace("Fix (", "Fix(");

    vba_lines_vector.push(vba_line_str);
}

fn translate_line(russian_line: String, vars: &mut HashMap<String, String>) -> String {
    let dictionary = get_keys_matrix();

    // __ Используем collect, чтобы не было проблем с временем жизни ссылки на russian_line
    let mut russian_line_mod = russian_line.clone();

    // __ Убираем двойные пробелы, потому что могут попадать одиночно стоящие цифры, когда разбиваем по пробелу
    while russian_line_mod.contains("  ") {
        russian_line_mod = russian_line_mod.replace("  ", " ");
    }
    let words: Vec<&str> = russian_line_mod.split_whitespace().collect();

    for word in words {
        // __ Если встречаем комментарий - выходим до конца строки
        if word.contains("'") {
            break;
        }

        // __ Еще раз проверяем на одиночные цифры
        let verify = word.trim().parse::<f64>();
        if verify.is_ok() {
            continue;
        }

        // __ Если это не ключевое слово
        if dictionary.get(word).is_none() {
            // __ Сразу создаем String нужной емкости (примерно в 1.5 раза больше исходного слова)
            let mut translit_result = String::with_capacity(word.len() * 2);

            for c in word.chars() {
                #[rustfmt::skip]
                let trans_c = match c {
                    ' ' | '(' | ')' | '-' | '+' => "_",
                    'а' => "a", 'А' => "A",
                    'б' => "b", 'Б' => "B",
                    'в' => "v", 'В' => "V",
                    'г' => "g", 'Г' => "G",
                    'д' => "d", 'Д' => "D",
                    'е' => "e", 'Е' => "E",
                    'ё' => "e", 'Ё' => "E",
                    'ж' => "zh", 'Ж' => "ZH",
                    'з' => "z", 'З' => "Z",
                    'и' => "i", 'И' => "I",
                    'й' => "j", 'Й' => "J",
                    'к' => "k", 'К' => "K",
                    'л' => "l", 'Л' => "L",
                    'м' => "m", 'М' => "M",
                    'н' => "n", 'Н' => "N",
                    'о' => "o", 'О' => "O",
                    'п' => "p", 'П' => "P",
                    'р' => "r", 'Р' => "R",
                    'с' => "s", 'С' => "S",
                    'т' => "t", 'Т' => "T",
                    'у' => "u", 'У' => "U",
                    'ф' => "f", 'Ф' => "F",
                    'х' => "h", 'Х' => "H",
                    'ц' => "c", 'Ц' => "C",
                    'ч' => "ch", 'Ч' => "Ch",
                    'ш' => "sh", 'Ш' => "Sh",
                    'щ' => "shch", 'Щ' => "SHCH",
                    'ъ' | 'ь'| 'Ъ' | 'Ь' => "",
                    'ы' => "y", 'Ы' => "Y",
                    'э' => "e", 'Э' => "E",
                    'ю' => "yu", 'Ю' => "YU",
                    'я' => "ya", 'Я' => "YA",
                    _ => {
                        // Для спецсимволов используем временный буфер,
                        // так как нам нужно превратить char в строку
                        translit_result.push(c); // __ Записываем символ
                        continue; // Переходим к следующему символу
                    }
                };

                translit_result.push_str(trans_c); // __ Записываем строку
            }

            // __ Записываем переменную в мапу переменных
            vars.insert(word.to_string(), translit_result);
            // Клонируем строку, чтобы использовать её и как ключ, и как значение
            // vars.insert(translit_result.clone(), translit_result);
        }
    }

    // __ Заменяем русские переменные на транслитерит
    // __ Тут сортируем по длине ключа по убыванию. Важно для правильной замены

    // __ 1. Превращаем в вектор кортежей (String, String)
    let mut sorted_vec: Vec<_> = vars.into_iter().collect();

    // __ 2. Сортируем по ключу (первый элемент кортежа) по убыванию
    sorted_vec.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

    let mut result = russian_line;
    for (rus_name, vba_name) in sorted_vec.iter() {
        // __ Тут столько танцев с бубнами, чтобы не менять на транслитерит то, что в комментариях совпадает с переменными
        if !result.contains("'") {
            result = result.replace(rus_name.as_str(), vba_name.as_str());
            continue;
        }

        let mut parts: Vec<String> = result.split('\'').map(|s| s.to_string()).collect();

        if parts.get(0).is_some() {
            parts[0] = parts[0].replace(rus_name.as_str(), vba_name.as_str());
        }

        result = parts.join("'");
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
