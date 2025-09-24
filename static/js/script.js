document.addEventListener("htmx:sseBeforeMessage", (e) => {
    console.log(e.detail.data)
})

let sortable

document.addEventListener('htmx:load', function (event) {
    console.log("onLoad")
    const sortableList = document.getElementById('pubs')
    sortable = new Sortable(sortableList, {
        animation: 150,
        disabled: true, // Disable dragging
    })

})

let debounceTimeout

document.addEventListener('htmx:afterSettle', function (evt) {
    // Clear any existing timeout
    clearTimeout(debounceTimeout)

    debounceTimeout = setTimeout(() => {
        console.log("Settle")
        const sortableList = document.getElementById('pubs')
        const cards = sortableList.children

        // Create an array of objects linking each card's ID to its input value
        const itemsToSort = Array.from(cards).map(card => {
            const id = card.dataset.id
            const input = card.querySelector('input[type="hidden"]')
            const value = parseInt(input.value, 10)
            return { id, value }
        })

        // Sort the array of objects based on the 'value' property
        itemsToSort.sort((a, b) => b.value - a.value)

        // Extract just the IDs from the sorted array into a new array
        const newOrder = itemsToSort.map(item => item.id)

        //TODO: Only run if order changes
        sortable.sort(newOrder, true)

    }, 150)
})
