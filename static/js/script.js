document.addEventListener("htmx:sse:before:message", (e) => {
    console.log(e.detail.message.data);
});

let sortable;

document.addEventListener("htmx:after:process", function () {
    const sortableList = document.getElementById("pubs");
    if (sortableList && sortable?.el !== sortableList) {
        sortable = new Sortable(sortableList, {
            animation: 150,
            disabled: true, // Disable dragging
        });
    }
});

let debounceTimeout;

document.addEventListener("htmx:after:settle", function () {
    clearTimeout(debounceTimeout);

    debounceTimeout = setTimeout(() => {
        const sortableList = document.getElementById("pubs");
        if (!sortableList) {
            return;
        }

        const cards = sortableList.querySelectorAll(".card");

        // Create an array of objects linking each card's ID to its input value
        const itemsToSort = Array.from(cards).map((card) => {
            const id = card.dataset.id;
            const input = card.querySelector('input[type="hidden"]');
            let value;
            try {
                value = parseInt(input.value);
            } catch {
                value = 0;
            }
            return { id, value };
        });

        if (itemsToSort.length > 0) {
            // Sort the array of objects based on the 'value' property
            itemsToSort.sort((a, b) => b.value - a.value);

            // Extract just the IDs from the sorted array into a new array
            const newOrder = itemsToSort.map((item) => item.id);

            //TODO: Only run if order changes
            sortable.sort(newOrder, true);
        }
    }, 150);
});
