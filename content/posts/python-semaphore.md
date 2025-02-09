+++
title = "Pythonda semaphore’ning vazifasi."
date = "2024-12-29"
+++

Parallelizmning eng katta muammosi resurslarni to'g'ri va xatosiz boshqarish, race condition/bottleneck holatlaridan qochishdir. Python parallelizmni boshqarish uchun turli yechimlar taqdim etadi, ularni orasida semaphore eng moslashuvchan hisoblanadi. U umumiy resurslardan multithreading/multiprocessing yoki asynchronous dasturlashda foydalanishni imkonini beradi. Boshlanishiga tushunish qiyin bo’lishi mumkin lekin post’ni oxirigacha o’qing va savollar bo’lsa telegram kanal comment’ida yozib qoldiring.

## Content:

1. Semaphore kelib chiqishi
2. Lock vs Semaphore
3. Semaphore’ni manual va context manager bilan yozish
4. Threading Semaphore
5. Asyncio Semaphore
6. Multithreading Semaphore
7. Xulosa

## 1. Semaphore kelib chiqishi

Semaphore tushunchasi 1960-yillarda mashhur kompyuter olimi **Edsger W. Dijkstra** tomonidan ishlab chiqilgan. U bu g'oyani operatsion tizimlardagi sinxronizatsiya muammolarini hal qilish uchun yaratgan. Bu kashfiyot bugungi zamonaviy parallelizmni boshqarish usullariga poydevor bo'ldi([Wikipedia](https://en.wikipedia.org/wiki/Semaphore_(programming)))

## 2. Lock vs Semaphore

Pythonda ikkala mexanizmning ham ishlash prinsipi bir xil - `.acquire()` va `.release()` metodlari mavjud va context manager'ni qo'llab-quvvatlaydi. Farqi shundaki, Semaphore'da biz turli thread'larning kirish huquqiga limit qo'ya olamiz, bunda Semaphore'ning standart qiymati 1 ga teng([docs](https://docs.python.org/3/library/threading.html#threading.Semaphore)).

## 3. Semaphore’ni manual va context manager bilan yozish
Manual:
```python
import threading
semaphore = threading.Semaphore(3)

semaphore.acquire()
try:
    # Logic
    print("Smth")
finally:
    semaphore.release()
```

Context manager bilan:
```python
import threading
semaphore = threading.Semaphore(3)

# Semafore'ni context manager bilan ishlatish
with semaphore:
    # Logic    
    print("Smth")

# Dasturni o'zi avtomatik .acquire() va .release() ni qo'yib ketadi.
```

## 4. Threading Semaphore

`threading` moduli `Semaphore` class’ini taqdim etadi. Bu klass ko'p oqimli dasturlarda umumiy resurslarni boshqarish uchun xizmat qiladi.

Semaphore nimaligini chunish uchun keling bir muammo ustida o’ylanib ko’raylik:

> Tasavvur qiling, bizda ofisda atigi 3 ta printer bor, lekin ishchilar soni 10 ta. Bu yerda cheklangan resurs - printer. Vazifamiz shundan iboratki, ishchilar hech qanday qiyinchiliksiz printerdan foydalanishi va undan foydalanib bo'lgach, boshqa ishchilarga printerni bo'shatib berishi kerak. Ushbu dasturni tuzish uchun Semaphore'dan foydalanamiz.

```python
import threading
import time
import random

class Printer:
    def __init__(self, printer_id):
        self.printer_id = printer_id

    def print_document(self, user_id):
        print(f"User-{user_id} is printing on Printer-{self.printer_id}.")
        time.sleep(random.randint(2, 5))  # random vaqt beramiz
        print(f"User-{user_id} has finished printing on Printer-{self.printer_id}.")

def user_action(user_id, printers, semaphore):
    print(f"User-{user_id} is trying to access the printers.")

    with semaphore:
        # random printer tanlaymiz
        printer = random.choice(printers)
        # printerni ishlatamiz
        printer.print_document(user_id)

def main():
    # 3ta printer yaratib olamiz
    printers = [Printer(printer_id=i) for i in range(3)]

    # Semaphore bilan 3ta odam access olishini berib ketamiz
    semaphore = threading.Semaphore(3)

    # 10 users (threads) yaratvolamiz
    users = [threading.Thread(target=user_action, args=(i, printers, semaphore)) for i in range(10)]

    # Start the threads
    for t in users:
        t.start()

    # Wait for all threads to finish
    for t in users:
        t.join()

if __name__ == "__main__":
    main()
```

Printerlar soni cheklangan bo'lgani uchun, semaphore nazoratchi sifatida ishlaydi - faqat belgilangan miqdordagi foydalanuvchilar (thread’lar) bir vaqtning o'zida printerlardan foydalana oladi, qolganlar esa kutishga majbur bo'ladi. Code’ni olib o’zingizda ishlatib ko’ring.

## 5. Asyncio Semaphore

Asynchronous dasturlashda `asyncio.Semaphore` xuddi threading'da o'rganganimiz kabi ishlaydi - u coroutine’larning ruxsat etilgan maksimal sonini nazorat qiladi. Bu ayniqsa API fetching/Scraping da foyda beradi. ([docs](https://docs.python.org/3/library/asyncio-sync.html#asyncio.Semaphore))
```python
import asyncio
import random

async def scrape_website(website, semaphore):
    async with semaphore:
        print(f"Scraping {website}...")
        await asyncio.sleep(random.randint(1, 3))
        print(f"Finished scraping {website}.")

async def main():
    # List of websites to scrape
    websites = [
        "https://example.com",
        "https://example.org",
        "https://example.net",
        "https://test.com",
        "https://demo.com",
        "https://sample.com",
    ]

    semaphore = asyncio.Semaphore(3)

    tasks = [scrape_website(website, semaphore) for website in websites]

    await asyncio.gather(*tasks)

asyncio.run(main())
```

## 6. Multiprocessing Semaphore

Huddi tepada o’qiganingiz kabi lekin endi process(CPU) bilan amalga oshiriladi va `multiprocessing.Semaphore` qilib threading’da o’rganganimiz kabi dasturni yozamiz.

## Xulosa

Semaphore Python'dagi parallelizm vositalari to'plamining muhim qismlaridan biri ekan va umumiy resurslarga kirishni boshqarishga yordam berarkan. Semaphore'ni qo'llanilishi tushinib, siz xavfsiz va samarali dasturlar yarata olarkansiz. Masalan, DB connection, rate-limiting va shunga o’xshash limit bor dasturlarni.

Kanalga obuna bo’ling: https://t.me/sayfulla_notes