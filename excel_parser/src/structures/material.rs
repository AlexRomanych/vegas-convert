use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Json;
use std::collections::HashMap;

#[derive(Default, Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Material {
    /// **Связь с Группой материалов - Группа материала (связь с ячейкой этой же таблицы)**
    pub material_group_code_1c: Option<String>,

    /// **Связь с Категорией материалов - Категория материала (связь с ячейкой этой же таблицы)**
    pub material_category_code_1c: Option<String>,

    /// **Код из 1С**
    pub code_1c: String,

    /// **Название материала**
    pub name: String,

    /// **Единица измерения**
    pub unit: Option<String>,

    /// **Поставщик**
    pub supplier: Option<String>,

    /// **Название объекта, к которому принадлежит материал (например, БлокПружинный)**
    pub object_name: Option<String>,

    /// **Сюда попадут все остальные характеристики: Длина, Ширина и т.д.**
    /// **Используем HashMap, чтобы сохранить оригинальные русские названия ключей**
    pub properties: Option<Json<HashMap<String, Value>>>,
}

impl Material {
    /// **Название таблицы процедур**
    pub const MATERIALS_TABLE_NAME: &'static str = "materials";

    // /// **Название файла Excel с материалами**
    // pub const MATERIALS_FILE_NAME: &'static str = "materials.xlsx"; - // __ Перенесли в constants

    /// **Стоп-слово окончания отчета ("Итого")**
    pub const STOP_WORD: &'static str = "Итого";

    /// **Номер строки начала данных**
    pub const DATA_START_ROW: usize = 7;

    /// **Номер столбца с кодом из 1С Группы материалов**
    pub const GROUP_CODE_COL: usize = 1;

    /// **Номер столбца с названием Группы материалов**
    pub const GROUP_NAME_COL: usize = 4;

    /// **Номер столбца с кодом из 1С Категории материалов**
    pub const CATEGORY_CODE_COL: usize = 6;

    /// **Номер столбца с названием Категории материалов**
    pub const CATEGORY_NAME_COL: usize = 8;

    /// **Номер столбца с кодом из 1С Материала**
    pub const MATERIAL_CODE_COL: usize = 9;

    /// **Номер столбца с названием Материала**
    pub const MATERIAL_NAME_COL: usize = 10;

    /// **Номер столбца с Единицей измерения**
    pub const UNIT_COL: usize = 11;

    /// **Номер столбца с названием Вида свойства**
    pub const PROPERTY_NAME_COL: usize = 14;

    /// **Номер столбца со значением Вида свойства**
    pub const PROPERTY_VALUE_COL: usize = 15;

    /// **Конструктор**
    pub fn new(code_1c: String, name: String) -> Self {
        Self {
            code_1c,
            name,
            material_group_code_1c: None,
            material_category_code_1c: None,
            unit: None,
            supplier: None,
            object_name: None,
            properties: None,
        }
    }

    /// **Проверяет, является ли объект пустым**
    pub fn is_empty(&self) -> bool {
        self.code_1c.is_empty() && self.name.is_empty()
    }

    /// **Сбрасывает объект**
    pub fn clear(&mut self) {
        self.code_1c = "".to_string();
        self.name = "".to_string();
        self.material_group_code_1c = None;
        self.material_category_code_1c = None;
        self.unit = None;
        self.supplier = None;
        self.object_name = None;
        self.properties = None;
    }
}

/*
public function up(): void
{
Schema::create(self::TABLE_NAME, function (Blueprint $table) {

//line -----------------------------------------------
//line ----- Для лучшей наглядности иерархии ---------
//line ----- такой порядок                   ---------
//line -----------------------------------------------
// Relations: Связь с Группой материалов
$table->string('material_group_code_1c', CODE_1C_LENGTH)
->nullable()
->comment('Группа материала');

// Relations: Связь с Категорией
$table->string('material_category_code_1c',CODE_1C_LENGTH)
->nullable()
->comment('Категория материала');
//line -----------------------------------------------

$table->string(CODE_1C, CODE_1C_LENGTH)->primary()->comment('Код 1C');
$table->string(CODE_1C . '_copy', CODE_1C_LENGTH)->nullable()->comment('Копия Кода 1C');

$table->string('name')->nullable(false)->comment('Название материала');
$table->string('unit')->nullable()->comment('Единица измерения');
// $table->string('unit')->nullable()->default(MaterialUnits::UNDEFINED)->comment('Единица измерения');
$table->string('supplier')->nullable()->comment('Поставщик');

$table->string('alt_unit')->nullable()->comment('Альтернативная Единица измерения');
$table->float('alt_multiplier')->nullable(false)->default(1.0)->comment('Альтернативная Единица измерения');
$table->boolean('apply_alt_unit')->nullable(false)->default(false)->comment('Применять альтернативную единицу измерения к материалу');

$table->unsignedInteger('order')->nullable(false)->default(0)->comment('Позиция в списке');

$table->boolean('is_deleted')->nullable(false)->default(false)->comment('Софт удаление');
$table->boolean('is_shown')->nullable(false)->default(true)->comment('Показывать в списке');
$table->boolean('is_collapsed')->nullable(false)->default(false)->comment('Схлопывать или разворачивать при запуске');
$table->boolean('is_checked')->nullable(false)->default(false)->comment('Проверено (для внутреннего использования)');

$table->softDeletes();

$table->jsonb('meta_extended')->nullable()->comment('Метаданные ++');
});

$this->addCommonColumns(self::TABLE_NAME);
$table->boolean('active')->nullable(false)->default(true)->comment('Актуальность');
$table->unsignedSmallInteger('status')->nullable()->comment('Статус');
$table->string('description')->nullable()->comment('Описание');
$table->string('comment')->nullable()->comment('Комментарий');
$table->string('note')->nullable()->comment('Примечание');
$table->json('meta')->nullable()->comment('Метаданные');
$table->string('color', 7)->default('#64748B')->comment('Цвет рендера');

'created_at'
'updated_at'


}
*/
