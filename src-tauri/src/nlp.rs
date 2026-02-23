// ============================================================================
// src-tauri/src/nlp.rs
// Модуль для извлечения сущностей из названий товаров (NLP)
// ============================================================================
// Дата: 10 февраля 2026
// Версия: 1.0.0
// Назначение: Извлечение брендов, моделей, категорий из raw_title
// ============================================================================

use regex::Regex;
use lazy_static::lazy_static;

// ============================================================================
// Структуры данных
// ============================================================================

#[derive(Debug, Clone)]
pub struct ExtractedEntities {
    pub normalized_title: String,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub category: Option<String>,
}

// ============================================================================
// Справочники и константы
// ============================================================================

// Список брендов электроники и бытовой техники (расширяемый)
const BRANDS: &[&str] = &[
    // Бытовая техника
    "Samsung", "LG", "Sony", "Panasonic", "Sharp", "Toshiba",
    "GE", "General Electric", "Whirlpool", "KitchenAid", 
    "Frigidaire", "Electrolux", "Bosch", "Miele",
    "Maytag", "Amana", "Jenn-Air", "Thermador", "Dacor",
    "Viking", "Wolf", "Sub-Zero", "Monogram",
    
    // Электроника
    "Apple", "Dell", "HP", "Hewlett Packard", "Lenovo", 
    "Asus", "Acer", "Microsoft", "Canon", "Nikon",
    "Bose", "JBL", "Harman Kardon", "Yamaha", "Denon",
    
    // Мебель
    "Ashley", "IKEA", "La-Z-Boy", "Ethan Allen", 
    "Pottery Barn", "West Elm", "Crate and Barrel",
    
    // Инструменты
    "DeWalt", "Milwaukee", "Makita", "Bosch", "Ryobi",
    "Craftsman", "Black & Decker", "Stanley",
];

// Стоп-слова (не несут смысловой нагрузки)
const STOP_WORDS: &[&str] = &[
    "new", "box", "open", "sealed", "ship", "retail",
    "brand", "factory", "original", "genuine", "authentic",
    "warranty", "refurbished", "renewed", "like", "condition",
    "lot", "pallet", "mixed", "assorted", "various",
];

// Категории по ключевым словам
const CATEGORIES: &[(&str, &[&str])] = &[
    ("Appliances", &["refrigerator", "fridge", "dishwasher", "washer", 
                     "dryer", "oven", "range", "stove", "microwave", 
                     "freezer", "cooktop"]),
    ("Electronics", &["tv", "television", "monitor", "laptop", "computer",
                      "tablet", "phone", "camera", "speaker", "headphone",
                      "soundbar", "receiver", "blu-ray", "dvd"]),
    ("Furniture", &["sofa", "couch", "chair", "table", "desk", "bed",
                    "dresser", "cabinet", "shelf", "bookcase", "nightstand"]),
    ("Tools", &["drill", "saw", "sander", "grinder", "wrench", "hammer",
                "tool set", "toolbox", "power tool", "hand tool"]),
    ("Home Decor", &["lamp", "mirror", "rug", "curtain", "pillow",
                     "artwork", "vase", "clock"]),
    ("Kitchen", &["blender", "mixer", "toaster", "coffee maker", "pot",
                  "pan", "knife set", "cookware"]),
];

// ============================================================================
// Regex паттерны для извлечения моделей
// ============================================================================

lazy_static! {
    // Паттерны для моделей различных брендов
    
    // Samsung: UN65TU8000FXZA, QN55Q80TAFXZA
    static ref SAMSUNG_MODEL: Regex = Regex::new(
        r"\b([UQ]N\d{2}[A-Z]+\d{2,5}[A-Z]*)\b"
    ).unwrap();
    
    // LG: OLED65C1PUB, 65NANO90UPA
    static ref LG_MODEL: Regex = Regex::new(
        r"\b(OLED\d{2}[A-Z0-9]+|[\d]{2}[A-Z]{4,}\d{2,}[A-Z]*)\b"
    ).unwrap();
    
    // GE: JVM3160RFSS, GNE27JSMSS
    static ref GE_MODEL: Regex = Regex::new(
        r"\b([A-Z]{3}\d{4}[A-Z]{2,4})\b"
    ).unwrap();
    
    // Общий паттерн: 2+ буквы + 3+ цифры + опционально буквы
    static ref GENERIC_MODEL: Regex = Regex::new(
        r"\b([A-Z]{2,}\d{3,}[A-Z0-9]*)\b"
    ).unwrap();
    
    // UPC/EAN коды (12-13 цифр)
    static ref UPC_CODE: Regex = Regex::new(
        r"\b(\d{12,13})\b"
    ).unwrap();
}

// ============================================================================
// EntityExtractor - основной класс
// ============================================================================

pub struct EntityExtractor {
    brands: Vec<String>,
    categories: Vec<(String, Vec<String>)>,
}

impl EntityExtractor {
    /// Создать новый экстрактор с предзагруженными справочниками
    pub fn new() -> Self {
        let brands = BRANDS.iter().map(|s| s.to_string()).collect();
        
        let categories = CATEGORIES.iter()
            .map(|(cat, keywords)| {
                let cat_string = cat.to_string();
                let keywords_vec = keywords.iter().map(|s| s.to_string()).collect();
                (cat_string, keywords_vec)
            })
            .collect();
        
        Self { brands, categories }
    }
    
    /// Главный метод: извлечь все сущности из названия
    pub fn extract(&self, raw_title: &str) -> ExtractedEntities {
        let normalized = self.normalize_title(raw_title);
        let brand = self.find_brand(&normalized);
        let model = self.find_model(raw_title); // Используем raw для regex
        let category = self.find_category(&normalized);
        
        ExtractedEntities {
            normalized_title: normalized,
            brand,
            model,
            category,
        }
    }
    
    // ========================================================================
    // Шаг 1: Нормализация названия
    // ========================================================================
    
    fn normalize_title(&self, title: &str) -> String {
        let mut result = title.to_lowercase();
        
        // Удаляем стоп-слова
        for stop_word in STOP_WORDS {
            let pattern = format!(r"\b{}\b", regex::escape(stop_word));
            if let Ok(re) = Regex::new(&pattern) {
                result = re.replace_all(&result, "").to_string();
            }
        }
        
        // Удаляем лишние пробелы
        result = result.split_whitespace().collect::<Vec<_>>().join(" ");
        
        // Удаляем специальные символы (оставляем буквы, цифры, пробелы)
        result = result.chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect();
        
        result.trim().to_string()
    }
    
    // ========================================================================
    // Шаг 2: Поиск бренда
    // ========================================================================
    
    fn find_brand(&self, normalized_title: &str) -> Option<String> {
        let lower = normalized_title.to_lowercase();
        
        // Ищем точное совпадение или вхождение
        for brand in &self.brands {
            let brand_lower = brand.to_lowercase();
            
            // Проверяем вхождение как целое слово
            let pattern = format!(r"\b{}\b", regex::escape(&brand_lower));
            if let Ok(re) = Regex::new(&pattern) {
                if re.is_match(&lower) {
                    return Some(brand.clone());
                }
            }
        }
        
        None
    }
    
    // ========================================================================
    // Шаг 3: Извлечение модели
    // ========================================================================
    
    fn find_model(&self, raw_title: &str) -> Option<String> {
        let upper = raw_title.to_uppercase();
        
        // Пробуем специфичные паттерны сначала
        if let Some(cap) = SAMSUNG_MODEL.captures(&upper) {
            return Some(cap[1].to_string());
        }
        
        if let Some(cap) = LG_MODEL.captures(&upper) {
            return Some(cap[1].to_string());
        }
        
        if let Some(cap) = GE_MODEL.captures(&upper) {
            return Some(cap[1].to_string());
        }
        
        // Пробуем общий паттерн
        if let Some(cap) = GENERIC_MODEL.captures(&upper) {
            let model = &cap[1];
            // Фильтруем очевидно неправильные (например, "NEW2024")
            if !model.starts_with("NEW") && !model.starts_with("BOX") {
                return Some(model.to_string());
            }
        }
        
        // Ищем UPC код как fallback
        if let Some(cap) = UPC_CODE.captures(&upper) {
            return Some(format!("UPC:{}", &cap[1]));
        }
        
        None
    }
    
    // ========================================================================
    // Шаг 4: Определение категории
    // ========================================================================
    
    fn find_category(&self, normalized_title: &str) -> Option<String> {
        let lower = normalized_title.to_lowercase();
        
        for (category, keywords) in &self.categories {
            for keyword in keywords {
                if lower.contains(keyword) {
                    return Some(category.clone());
                }
            }
        }
        
        None
    }
}

// ============================================================================
// Дополнительные утилиты
// ============================================================================

/// Извлечь размер (дюймы) из названия TV/монитора
pub fn extract_screen_size(title: &str) -> Option<u32> {
    let re = Regex::new(r#"\b(\d{2,3})[\"'\s]?(inch|in|tv|television|monitor)?\b"#).ok()?;
    
    if let Some(cap) = re.captures(&title.to_lowercase()) {
        if let Ok(size) = cap[1].parse::<u32>() {
            // Разумные размеры экранов: 15-100 дюймов
            if (15..=100).contains(&size) {
                return Some(size);
            }
        }
    }
    
    None
}

/// Извлечь вместимость (cubic feet) для холодильников
pub fn extract_capacity(title: &str) -> Option<f64> {
    let re = Regex::new(r"(\d+\.?\d*)\s*(cu\.?\s*ft|cubic\s*feet?)").ok()?;
    
    if let Some(cap) = re.captures(&title.to_lowercase()) {
        if let Ok(capacity) = cap[1].parse::<f64>() {
            // Разумные вместимости: 1-30 cubic feet
            if (1.0..=30.0).contains(&capacity) {
                return Some(capacity);
            }
        }
    }
    
    None
}

// ============================================================================
// Тесты
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_normalize_title() {
        let extractor = EntityExtractor::new();
        
        let input = "NEW Samsung 65\" 4K UHD Smart TV - Open Box";
        let normalized = extractor.normalize_title(input);
        
        assert!(!normalized.contains("new"));
        assert!(!normalized.contains("open"));
        assert!(normalized.contains("samsung"));
        assert!(normalized.contains("65"));
    }
    
    #[test]
    fn test_find_brand() {
        let extractor = EntityExtractor::new();
        
        let cases = vec![
            ("Samsung 65\" TV", Some("Samsung")),
            ("LG OLED55C1PUB", Some("LG")),
            ("GE Profile Microwave", Some("GE")),
            ("Unknown Brand TV", None),
        ];
        
        for (input, expected) in cases {
            let normalized = extractor.normalize_title(input);
            let result = extractor.find_brand(&normalized);
            
            match expected {
                Some(brand) => assert_eq!(result, Some(brand.to_string())),
                None => assert_eq!(result, None),
            }
        }
    }
    
    #[test]
    fn test_find_model_samsung() {
        let extractor = EntityExtractor::new();
        
        let input = "Samsung UN65TU8000FXZA 65\" 4K UHD TV";
        let model = extractor.find_model(input);
        
        assert_eq!(model, Some("UN65TU8000FXZA".to_string()));
    }
    
    #[test]
    fn test_find_model_lg() {
        let extractor = EntityExtractor::new();
        
        let input = "LG OLED65C1PUB 65\" OLED TV";
        let model = extractor.find_model(input);
        
        assert_eq!(model, Some("OLED65C1PUB".to_string()));
    }
    
    #[test]
    fn test_find_model_ge() {
        let extractor = EntityExtractor::new();
        
        let input = "GE Profile JVM3160RFSS Over-the-Range Microwave";
        let model = extractor.find_model(input);
        
        assert_eq!(model, Some("JVM3160RFSS".to_string()));
    }
    
    #[test]
    fn test_find_category() {
        let extractor = EntityExtractor::new();
        
        let cases = vec![
            ("Samsung Refrigerator", Some("Appliances")),
            ("65\" TV Television", Some("Electronics")),
            ("Leather Sofa", Some("Furniture")),
            ("DeWalt Drill Set", Some("Tools")),
            ("Mystery Item", None),
        ];
        
        for (input, expected) in cases {
            let normalized = extractor.normalize_title(input);
            let result = extractor.find_category(&normalized);
            
            match expected {
                Some(cat) => assert_eq!(result, Some(cat.to_string())),
                None => assert_eq!(result, None),
            }
        }
    }
    
    #[test]
    fn test_extract_screen_size() {
        assert_eq!(extract_screen_size("Samsung 65\" TV"), Some(65));
        assert_eq!(extract_screen_size("55 inch Monitor"), Some(55));
        assert_eq!(extract_screen_size("No size here"), None);
        assert_eq!(extract_screen_size("999 inch invalid"), None); // too large
    }
    
    #[test]
    fn test_extract_capacity() {
        assert_eq!(extract_capacity("25 cu. ft. Refrigerator"), Some(25.0));
        assert_eq!(extract_capacity("18.5 cubic feet Fridge"), Some(18.5));
        assert_eq!(extract_capacity("No capacity"), None);
    }
    
    #[test]
    fn test_full_extraction() {
        let extractor = EntityExtractor::new();
        
        let title = "GE Profile Spacemaker 1.9 cu ft OTR Microwave JVM3160RFSS";
        let entities = extractor.extract(title);
        
        assert_eq!(entities.brand, Some("GE".to_string()));
        assert_eq!(entities.model, Some("JVM3160RFSS".to_string()));
        assert_eq!(entities.category, Some("Appliances".to_string()));
        assert!(entities.normalized_title.contains("profile"));
    }
}

// ============================================================================
// Экспорт для использования в main.rs
// ============================================================================

impl Default for EntityExtractor {
    fn default() -> Self {
        Self::new()
    }
}
