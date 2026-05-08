use crate::structures::expression_nodes::ExpressionNode;
use crate::structures::tokens::Token;
use procedures::structures::procedure::Procedure;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Default)]
pub struct ParsedProcedure {
    pub procedure:        Procedure,
    pub tokens:           Vec<Token>,
    pub expressions_node: ExpressionNode,
    pub returns_raw:      HashMap<String, f64>,
    pub returns:          HashMap<String, f64>,
    pub properties_raw:   HashMap<String, f64>, // raw - это сырые значения: [Матрас].[Длина]
    pub properties:       HashMap<String, f64>, // Без raw - это значения без [] и родителя: Длина
    pub parameters_raw:   HashMap<String, f64>,
    pub parameters:       HashMap<String, f64>,
    pub outputs_raw:      BTreeMap<String, f64>, // В отсортированном порядке
    pub outputs:          BTreeMap<String, f64>, // Выходные параметры в отсортированном порядке
    pub in_scope:         HashMap<String, f64>,  // Входные параметры, которые не меняются в процессе расчетов
    pub out_scope:        HashMap<String, f64>,  // Все переменные, которые получились в результате расчетов в процедуре

    pub has_properties: bool, // __ Есть ли свойства: [НастилМатериалы].{Плотность}
    pub has_parameters: bool, // __ Есть ли параметры: Ширина = [Матрас].[Ширина]
}

impl ParsedProcedure {
    // __ Печатаем токены. Используем для отладки
    pub fn print_tokens(&self) {
        self.tokens
            .iter()
            .enumerate() // Добавляет счетчик (0, 1, 2...)
            .for_each(|(i, token)| {
                println!("{i}: {token:?}");
            });
    }


    // __ Очищаем от скобочек [] то, что нашли
    pub fn un_raw(&mut self) {
        self.returns = Self::process_list(&self.returns_raw);
        self.properties = Self::process_list(&self.properties_raw);
        self.parameters = Self::process_list(&self.parameters_raw);
        self.outputs = Self::process_list(&self.outputs_raw);
    }

    // __ Очищаем от скобочек [] выходные параметры
    pub fn un_raw_outputs(&mut self) {
        self.outputs = Self::process_list(&self.outputs_raw);
    }

    fn remove_pair(text: &str) -> String {
        // Сначала берем часть после точки, если она есть
        let target = text
            .split_once('.')
            .map(|(_, p2)| p2)
            .unwrap_or(text);
        target
            .replace('[', "")
            .replace(']', "")
            .replace('{', "")
            .replace('}', "")
    }

    fn process_list<T, V>(list: &T) -> T
    where
        // 1. Ссылка на T должна давать итератор по (String, V)
        for<'a> &'a T: IntoIterator<Item = (&'a String, &'a V)>,
        // 2. T должен уметь собираться из (String, V)
        T: FromIterator<(String, V)>,
        // 3. Тип значения V должен поддерживать клонирование
        V: Clone,
    {
        list.into_iter()
            .map(|(k, v)| (Self::remove_pair(k), v.clone()))
            .collect()
    }

    // __ Устанавливаем входные Scopes
    pub fn set_scopes(&mut self, scopes: &Vec<(String, f64)>) {
        for (var, val) in scopes {
            // __ Вставляем входные параметры
            if let Some(v) = self.parameters.get(var) {
                self.parameters
                    .insert(var.clone(), *val);
            }

            // __ !!! Работаем с этим набором данных
            // __ Вставляем входные параметры в оригинальные названия парметров после паринга токенов [Матрас].[Длина]
            // __ Приходят только в виде вектора кортежей ("Длина", 2.0)
            for (k, v) in self.parameters_raw.iter_mut() {
                *v = 0.0;   // Обнуляем
                for (parameter, value) in scopes {
                    if k.contains(parameter) {
                        *v = *value;
                        break;
                    }
                }
            }

            // __ Вставляем входные свойства
            if let Some(v) = self.properties.get(var) {
                self.properties
                    .insert(var.clone(), *val);
            }

            // __ !!! Работаем с этим набором данных
            // __ Вставляем входные свойства в оригинальные названия парметров после паринга токенов [Матрас].[Длина]
            // __ Приходят только в виде вектора кортежей ("Длина", 2.0)
            for (k, v) in self.properties_raw.iter_mut() {
                *v = 0.0;   // Обнуляем
                for (parameter, value) in scopes {
                    if k.contains(parameter) {
                        *v = *value;
                        break;
                    }
                }
            }
        }
    }

    // __ Получаем итоговые результаты: результат вычислений + отход и заполняем соответствующий массив
    // TODO: Переделать поиск по полю object_name - [БлокПружинный] и [БлокПружинныйОтход]
    pub fn set_results(&mut self, scopes: &HashMap<String, f64>) -> (f64, Option<f64>) {
        let mut procedure_result = 0.0;
        let mut procedure_rest: Option<f64> = None;

        let mut is_rest_present = false;
        for (k, v) in self.returns_raw.iter_mut() {
            *v = 0.0;
            if let Some(value) = scopes.get(k) {
                *v = *value;
                if k.to_lowercase().contains("отход") {
                    is_rest_present = true;
                    procedure_rest = Some(*value);
                } else {
                    procedure_result = *value;
                }

            }
        }
        (procedure_result, procedure_rest)
    }

    // __ Получаем выходные параметры
    pub fn set_outputs(&mut self, scopes: &HashMap<String, f64>) {
        for (k, v) in self.outputs_raw.iter_mut() {
            *v = 0.0;
            if let Some(value) = scopes.get(k) {
                *v = *value;
            }
        }

        Self::un_raw_outputs(self);
    }


}
