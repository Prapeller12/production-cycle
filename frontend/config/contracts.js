'use strict';
window.Production = window.Production || {};
(() => {
const OUT_HEADER=['Количество связей, поясняющих суть документа','Регистрационный номер','Дата','Получатель исходящего','Заголовок','Подписант','Срок исполнения','Адресаты','Ссылка'];
const IN_HEADER=['Количество связей, поясняющих суть документа','Краткое содержание','Рег. номер','Дата регистрации','Корреспондент','Адресаты','Срок исполнения','Ссылка'];
const STATUS={new:'Не начато',work:'В работе',hold:'Приостановлено',done:'Выполнено'};
Production.config=Object.freeze({OUT_HEADER:Object.freeze(OUT_HEADER),IN_HEADER:Object.freeze(IN_HEADER),STATUS:Object.freeze(STATUS),dueSoonDays:7,formats:['production-cycle-builder-v3','production-cycle-builder-v5','production-cycle-builder-v6','production-cycle-builder-v8']});
})();
