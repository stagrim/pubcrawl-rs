let debounceTimeout;

// Sort pubs after an update
document.addEventListener("htmx:after:settle", function () {
    clearTimeout(debounceTimeout);

    debounceTimeout = setTimeout(() => {
        const pubsList = document.getElementById("pubs");
        if (!pubsList) {
            return;
        }

        const cards = Array.from(pubsList.querySelectorAll(".card"));
        if (cards.length < 2) {
            return;
        }

        const itemsToSort = cards.map((card) => {
            const input = card.querySelector('input[type="hidden"]');
            const value = input ? parseInt(input.value, 10) || 0 : 0;
            return { card, id: card.dataset.id, value };
        });

        itemsToSort.sort((a, b) => b.value - a.value);

        const orderChanged = itemsToSort.some(
            (item, index) => item.card !== cards[index],
        );
        if (!orderChanged) {
            return;
        }

        const firstTops = new Map();
        for (const card of cards) {
            firstTops.set(card, card.getBoundingClientRect().top);
        }

        for (const { card } of itemsToSort) {
            pubsList.appendChild(card);
        }

        for (const { card } of itemsToSort) {
            const deltaY =
                firstTops.get(card) - card.getBoundingClientRect().top;
            if (deltaY !== 0) {
                card.animate(
                    [
                        { transform: `translateY(${deltaY}px)` },
                        { transform: "translateY(0)" },
                    ],
                    {
                        duration: 150,
                        easing: "ease",
                    },
                );
            }
        }
    }, 150);
});
