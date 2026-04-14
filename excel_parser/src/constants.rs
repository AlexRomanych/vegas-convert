/// **Название листа в отчетах из 1С**
pub const DATA_SHEET_1C_NAME: &str = "TDSheet";


/// **Длина кода из 1С**
pub const CODE_1C_LENGTH: usize = 9;


/// **Путь к отчетам**
pub const IMPORT_PATH: &str = "storage/app/1c_imports/";

/// **Название файла Excel с материалами**
pub const MATERIALS_FILE_NAME: &'static str = "materials.xlsx";

/// **Название файла Excel с материалами**
pub const PROCEDURES_FILE_NAME: &'static str = "procedures.xlsx";

/// **Название файла Excel с моделями**
pub const MODELS_FILE_NAME: &'static str = "models.xlsx";

/// **Название файла Excel со спецификациями**
pub const SPECIFICATIONS_FILE_NAME: &'static str = "specifications.xlsx";


/// **Название таблицы с коллекциями**
pub const MODEL_COLLECTIONS_TABLE_NAME: &'static str = "model_collections";

/// **Название таблицы со Статусами производства (Выпускается, Вариант исполнения, ...)**
pub const MODEL_MANUFACTURE_STATUSES_TABLE_NAME: &'static str = "model_manufacture_statuses";

/// **Название таблицы с Видами производства (Производство матрасов, Производство постельного белья, ...)**
pub const MODEL_MANUFACTURE_TYPES_TABLE_NAME: &'static str = "model_manufacture_types";

/// **Название таблицы с Типами изделий (Матрас, Чехол, Подушка, ...)**
pub const MODEL_TYPES_TABLE_NAME: &'static str = "model_types";

/// **Название таблицы с Группами сортировки (Обшивка-Скрутка, Неопознанные, FMX, ...)**
pub const MODEL_MANUFACTURE_GROUPS_TABLE_NAME: &'static str = "model_manufacture_groups";


/// **Название Группы пропущенных материалов спецификаций**
pub const MISSING_MATERIALS_GROUP_NAME: &str = "Отсутствующее сырье из спецификаций";

/// **Код Группы пропущенных материалов спецификаций**
pub const MISSING_MATERIALS_GROUP_CODE_1C: &str = "GR_MS_000";

/// **Название Категории пропущенных материалов спецификаций**
pub const MISSING_MATERIALS_CATEGORY_NAME: &str = "Наименование материалов";

/// **Код Категории пропущенных материалов спецификаций**
pub const MISSING_MATERIALS_CATEGORY_CODE_1C: &str = "CT_MS_000";
