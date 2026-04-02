use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
// use sqlx::types::{
//     Json,
//     chrono::{DateTime, Utc},
// };
// use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Model {
    pub code_1c: String, // __ Код по 1С (Primary Key) 'Код по 1С.1'

    // Relations: Внешние ключи - i64
    pub model_manufacture_status_id: Option<i64>, // __ Статус Код: Выпускается, Архив, Вариант исполнения, ... 'Порядок, Статус.2 (Выпускается, Архив, ...)'
    pub model_manufacture_group_id:  i64, // __ Группа сортировки: FMX, Обшивка-Скрутка, ... По дефолту 0, не null 'Номер группы модели для сортировки.31'

    // Relations: Внешние ключи - String
    pub model_collection_code_1c:       Option<String>, // __ Коллекция 'Коллекция.Код.3'
    pub model_type_code_1c:             Option<String>, // __ Тип модели: Матрас, Наматрасник, Подушка, ... 'Тип продукции.Код.5 (Матрас, Наматрасник, ...)'
    pub model_manufacture_type_code_1c: Option<String>, // __ Тип производства: Производство матрасов, Производство подушек, ... 'Вид производства.Код.41 (Производство матрасов, Производство стеганных полотен ...)'
    pub cover_code_1c:                  Option<String>, // Relations: 'Модель чехла.Код.12'
    pub cover_code_1c_copy:             Option<String>, // Relations: 'Копия Модель чехла.Код.12'

    // ___ Поля, которых нет в таблице, но они нужны для логики работы парсера
    pub model_manufacture_status_name: Option<String>, // __ Статус Наименование: Выпускается, Архив, Вариант исполнения, ... 'Порядок, Статус.2 (Выпускается, Архив, ...)'
    pub model_collection_name:         Option<String>, // __ Коллекция 'Коллекция.Наименование.4'
    pub model_type_name:               Option<String>, // __ Тип модели: Матрас, Наматрасник, Подушка, ... 'Тип продукции.Наименование.6 (Матрас, Наматрасник, ...)'
    pub model_manufacture_type_name:   Option<String>, // __ Тип производства: Производство матрасов, Производство подушек, ... 'Вид производства.Наименование.42 (Производство матрасов, Производство стеганных полотен ...)'

    // ___ Текстовые поля
    pub serial:              Option<String>, // __ 'Серия.7'
    pub name:                String,         // __ 'Наименование.8' // nullable(false)
    pub name_short:          Option<String>, // __ 'Наименование краткое.9'
    pub name_common:         Option<String>, // __ 'Наименование общее.10'
    pub name_report:         Option<String>, // __ 'Имя отчеты.11'
    pub cover_name_1c:       Option<String>, // __ 'Модель чехла.Наименование.13'
    pub textile:             Option<String>, // __ 'Ткань.16'
    pub textile_composition: Option<String>, // __ 'Состав ткани.17'
    pub cover_type:          Option<String>, // __ 'Тип чехла.Наименование.18'
    pub zipper:              Option<String>, // __ 'Молния.19'
    pub spacer:              Option<String>, // __ 'Прокладочный материал.Наименование.20'
    pub stitch_pattern:      Option<String>, // __ 'Рисунок стежки. Наименование.21'
    pub pack_type:           Option<String>, // __ 'Вид упаковки.Наименование.22'
    pub base_composition:    Option<String>, // __ 'Состав мягкого элемента.23'
    pub side_foam:           Option<String>, // __ 'ППУ бортов.24'
    pub base_block:          Option<String>, // __ 'Базовый блок.25'
    pub cover_mark:          Option<String>, // __ 'Маркировка чехла.29'
    pub model_mark:          Option<String>, // __ 'Маркировка матраса.30'
    pub owner:               Option<String>, // __ 'Владелец.32'
    pub sewing_machine:      Option<String>, // __ 'Группы ДСЗ.34 (Тип швейных машин: АШМ, УШМ, Обшивка и Прочее)'
    pub kant:                Option<String>, // __ 'Кант.35'
    pub tkch:                Option<String>, // __ 'ТКЧ.36 (Типовая конструкция чехла)'
    pub side_height:         Option<String>, // __ 'Высота бортов.38'
    pub barcode:             Option<String>, // __ 'Штрих код.44'
    // pub description:         Option<String>, // __ Описание
    // pub comment:             Option<String>, // __ Комментарий
    // pub note:                Option<String>, // __ Примечание

    // ___ Характеристики (Decimal в Rust лучше всего мапится на rust_decimal::Decimal)
    pub base_height:    Decimal,         // __ 'Стандартная высота.14 (высота матраса, м)' nullable(false)
    pub cover_height:   Decimal,         // __ 'Стандартная высота чехла.15 (высота чехла, м)' nullable(false)
    pub pack_density:   Option<Decimal>, // __ 'Плотность упаковки.37'
    pub pack_weight_rb: Option<Decimal>, // __ 'Вес упаковки РБ.39'
    pub pack_weight_ex: Option<Decimal>, // __ 'Вес упаковки экспорт.40'
    pub weight:         Decimal,         // __ 'Вес в г.43 (куб. см. ???)'

    // ___ Числовые характеристики
    pub load:      Option<i32>, // __ 'Нагрузка.26' unsignedInteger -> i32
    pub guarantee: Option<i32>, // __ 'Гарантийный срок, мес.27' unsignedInteger
    pub life:      Option<i32>, // __ 'Срок службы, лет.28' unsignedInteger
    // pub status:    Option<i16>, // __ Статус unsignedSmallInteger -> i16

    // ___ Флаги (Boolean)
    pub lamit:  Option<bool>, // __ 'Возможность изготовления на линии (Lamit).33'
    // pub active: bool,         // __ 'Активный или Архивный.45' default true, nullable(false)

    // ___ JSONB Поля (Спецификации)
    // ___ Используем serde_json::Value для гибкости или конкретные структуры
    // pub base:  Option<Json<serde_json::Value>>, // __ 'Спецификация МЭ'
    // pub cover: Option<Json<serde_json::Value>>, // __ 'Спецификация чехла'
    // pub meta:  Option<Json<serde_json::Value>>, // __ 'Метаданные'

    // ___ Timestamps
    // pub created_at: Option<DateTime<Utc>>,
    // pub updated_at: Option<DateTime<Utc>>,
}

impl Model {
    /// **Название таблицы моделей**
    pub const MODELS_TABLE_NAME: &'static str = "models";

    /// **Номер строки начала данных**
    pub const DATA_START_ROW: usize = 5;

    pub const CODE_1C_COL: usize = 1;
    pub const MODEL_MANUFACTURE_STATUS_COL: usize = 2;
    pub const MODEL_COLLECTION_CODE_1C_COL: usize = 3;
    pub const MODEL_COLLECTION_NAME_COL: usize = 4;
    pub const MODEL_TYPE_CODE_1C_COL: usize = 5;
    pub const MODEL_TYPE_NAME_COL: usize = 6;
    pub const MODEL_SERIAL_COL: usize = 7;

    /// __ 'Наименование.8'
    pub const NAME_COL: usize = 8;

    // __ 'Наименование краткое.9'
    pub const NAME_SHORT_COL: usize = 9;

    // __ 'Наименование общее.10'
    pub const NAME_COMMON_COL: usize = 10;

    // __ 'Имя отчеты.11'
    pub const NAME_REPORT_COL: usize = 11;

    pub const COVER_CODE_1C_COL: usize = 12;
    pub const COVER_NAME_1C_COL: usize = 13;

    pub const BASE_HEIGHT_COL: usize = 14;
    pub const COVER_HEIGHT_COL: usize = 15;

    pub const TEXTILE_COL: usize = 16;
    pub const TEXTILE_COMPOSITION_COL: usize = 17;
    pub const COVER_TYPE_COL: usize = 18;
    pub const ZIPPER_COL: usize = 19;
    pub const SPACER_COL: usize = 20;
    pub const STITCH_PATTERN_COL: usize = 21;
    pub const PACK_TYPE_COL: usize = 22;
    pub const BASE_COMPOSITION_COL: usize = 23;
    pub const SIDE_FOAM_COL: usize = 24;
    pub const BASE_BLOCK_COL: usize = 25;
    pub const LOAD_COL: usize = 26;
    pub const GUARANTEE_COL: usize = 27;
    pub const LIFE_COL: usize = 28;
    pub const COVER_MARK_COL: usize = 29;
    pub const MODEL_MARK_COL: usize = 30;
    pub const MODEL_MANUFACTURE_GROUP_ID_COL: usize = 31;
    pub const OWNER_COL: usize = 32;
    pub const LAMIT_COL: usize = 33;
    pub const SEWING_MACHINE_COL: usize = 34;
    pub const KANT_COL: usize = 35;
    pub const TKCH_COL: usize = 36;
    pub const PACK_DENSITY_COL: usize = 37;
    pub const SIDE_HEIGHT_COL: usize = 38;
    pub const PACK_WEIGHT_RB_COL: usize = 39;
    pub const PACK_WEIGHT_EX_COL: usize = 40;

    pub const MODEL_MANUFACTURE_TYPE_CODE_1C_COL: usize = 41;
    pub const MODEL_MANUFACTURE_TYPE_NAME_COL: usize = 42;

    pub const WEIGHT_COL: usize = 43;
    pub const BARCODE_COL: usize = 44;
}
