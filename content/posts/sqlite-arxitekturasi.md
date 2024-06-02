+++
title = "Sqlite arxitekturasi/qanday ishlashi"
date = "2024-06-02"
+++

Sqlite bu yagona file'da ma'lumotlarni saqlay oluvchi minimal [RDBMS](https://en.wikipedia.org/wiki/Relational_database) va u SQL text'larni bytecode'larda "virtual machine" yaratish orqali saqlaydi.

Ushbu rasmda sqlite'ni qanday ishlashi haqida qisqacha tushuncha olishingiz mumkin:
![Sqlite image](/images/sqlite.png)


# Interface
SQL buyruqlarini qabul qiluvchi qatlam bo'lib userga ko'rinadigan barcha narsani/result'ni shu yerga kiritishimiz mumkin.
Asosiy componentlariga kiritishimiz mumkin:
- API functions: ma'lumotlar bazasiga ulanish, SQL buyruqlarini o'qish/bajarish/natijalarni olishni va h.k.z.
- Error handling: hosil bo'ladigan, kelib chiqadigan xatoliklarni boshqarish.

# SQL command protsessor
Bu "SQL query"larni [Virtual Machine](https://en.wikipedia.org/wiki/Virtual_machine)ga tayyorlashga yordam beruvchi qatlam. Uning o'zi yana bo'linadi:
## Tokenizer: 
SQL buyruqni kichkina token'larga bo'lakchalarga bo'lib chiqadi va bunda asosiy e'tibor yozilgan buyruqqa qaratiladi ortiqcha bo'shliq(whitespace)lar olib tashlanadi.
- Lexical Analysis: SQL buyruq uzun matn ko'rinishda kelsa bo'lakchalarga buyruqlarga bo'ladi
- Token Classification: Buyruqni turini qandaydir kalit so'zligini aniqlab beradi
- Handling Whitespace and Comments: Aytganimizdek keraksiz sharxlarni, bo'shliqlarni olib tashlaydi.

Keling endi bir misol ko'rsak. Bizda ushbu buyruq bor: `SELECT name FROM users WHERE age > 21;`
Tokenizer ushbu ko'rinishga keltirib oladi:
- SELECT (keyword)
- name (identifier)
- FROM (keyword)
- users (identifier)
- WHERE (keyword)
- age (identifier)
- `>` (operator)
- 21 (literal)

## Parser: 
Tokenizer'dan kelgan natijani olib [parse tree](https://en.wikipedia.org/wiki/Parse_tree)ga o'girib oladi. Vazifasi sintaksisni analiz qiladi va xatolik bo'lsa u haqida ma'lumot beradi.

## Code generator: 
Parserdan kelgan "parse tree"ni oladi. "Bytecode"ga o'girgan xolda amallarni bajaradi.
Misol tariqasida ushbu so'rovni ko'rsak: `SELECT name FROM users WHERE age > 21;`
1. `users` nomli table'ni ochadi.
2. `age > 21` bo'yicha filter qiladi.
3. Filter bo'lgan ma'lumotlardan `name`ni ajratib oladi.
4. Natijani qaytaradi.

# Virtual Machine (VM)
O'z nomi bilan machine yaratib oladi "bytecode"ni qabul qilib, amalda bajaradi. Quyi darajadagi barcha narsa kirish/chiqish kabilarni o'z qamroviga oladi.

# B-Tree
[B-Tree (Balanced Tree)](https://en.wikipedia.org/wiki/B-tree) bu ma'lumot tuzilmasi bo'lib asosan [table](https://en.wikipedia.org/wiki/Table_(database)) va [index](https://en.wikipedia.org/wiki/Database_index)'larni saqlashda ishlatiladi. Balanced muvozanatda bo'lganligi tufayli qidiruv, ma'lumot qo'shish va o'chirishni O(log n)da bajarish imkonini beradi.

# Pager
Diskdan o'qib va yozish kabilarni havfsiz o'tishiga javob beradi ya'nikim [ACID](https://en.wikipedia.org/wiki/ACID). Oxirgi kirish/chiqish amallarini cache'da saqlab qoladi, transaction'larni amalga oshirish kabilar shu qatlamda bo'ladi. Sinxron ishlash kerakli amallarni ketma-ketligiga shu qism javobgardir.

# OS interface
Operatsion tizim darajasidagi amallarga javob beradi, resurslarni boshqaruvchi qatlam. File ochish/yopish kabilarga, xotirani boshqarishga, tizim darajasidagi xatoliklarni aniqlashga, amallarni turli operatsion tizimlarda to'g'ri ishlashiga shu qatlam javob beradi.

# Utilities
Kichik vazifalarni bajaradigan barcha funksiyalar, kengaytmalar shu yerda bo'ladi.

# Test Code
Oz nomi bilan testlarni qamrab oladi bunda biz yozgan code to'g'ri ishlayotgani, to'g'ri amallarni bajarayotganini yoki yo'qligini bilishga yordam beradi.

Obuna bo'ling: https://t.me/sayfulla_blog
