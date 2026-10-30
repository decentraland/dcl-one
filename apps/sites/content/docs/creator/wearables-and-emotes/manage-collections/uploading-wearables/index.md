> Guidelines to Upload Wearables to the Editor

# Uploading Wearables

Once you export your wearable, you’ll have to upload it to the builder. This document will cover the process of uploading wearables.

## Uploading Your File

Remember that you need to create a collection before you can upload your file. If you don’t know how to do that, check [Creating a Collection](/docs/creator/wearables-and-emotes/manage-collections/creating-a-collection/). To upload your wearable, just drag and drop the file on the _**New Item**_ window or browse your computer. It will automatically detect if the file is an emote or wearable. **Remember that the collection max file size is 3MB**.

![](</docs/.gitbook/assets/01_new_item (2).webp>)

When you upload the file, you will be asked to select a body shape, enter a name and define the rarity and the category. You can also add the thumbnail for the wearable.

### **Body Shape**

To ensure that your wearables can be worn by the intended avatars, you need to upload separate GLB files for each body shape. If you have two separate versions of the wearable, one for male and one for female, you can add one of the representations during the upload process and then add the other later using the editor. If your wearable is meant to be unisex, make sure to upload a single GLB file that is designed to fit both male and female versions.

![](/docs/.gitbook/assets/03_body_shape.webp)

### **Rarity**

Select the Rarity of your item.

![](/docs/.gitbook/assets/04_rarity.webp)

| Rarity    | Number of Items |
| --------- | --------------- |
| Unique    | 1               |
| Mythic    | 10              |
| Exotic    | 50              |
| Legendary | 100             |
| Epic      | 1,000           |
| Rare      | 5,000           |
| Uncommon  | 10,000          |
| Common    | 100,000         |

### **Category**

Wearables are organized into different categories, depending on what part of an avatar they modify. Select the appropriate category for your item.

![](/docs/.gitbook/assets/05_category.webp)

### **Custom Thumbnails**

You can add your own custom thumbnail by clicking on the camera icon and browsing your computer. **The thumbnail must be 256px square .png file with transparent background.** Collections containing thumbnails without transparent backgrounds will not be accepted by the Curation Committee.

![](</docs/.gitbook/assets/06_thumbnail (1).webp>)


> [!WARNING]
> ⚠️ Having a good render of your wearable is crucial in making it more appealing to potential users in the marketplace. **It's important to avoid adding any graphics other than the wearable itself, because this may cause the curation committee to reject it.**
> 
> <img src="/docs/.gitbook/assets/07_thumbnail (1).webp" alt="" data-size="original">


### **Spring Bones**

If your wearable contains bones with `springbone` in their name, the Builder will automatically detect them and display a **Spring Bones** configuration panel. From this panel you can adjust physics parameters like stiffness, gravity, and drag for each spring bone chain. See [Spring Bones](/docs/creator/wearables-and-emotes/wearables/spring-bones/) for details on how to set up spring bones in your model.

### **Properties**

Below the thumbnail you're going to find the properties of your wearable, number of triangles of your model, number of materials and textures.

![](</docs/.gitbook/assets/08_properties (1).webp>)

## **Uploading Mouth, Eyes and Eyebrows**

The mouth, eyes and eyebrows category have a different behaviour in the editor because these are just .png files. To upload these just drag and drop the png file as a transparent image (256X256 pixels). Mouth is going to be automatically tinted by skin color, same for the eyebrows tinted by the hair color.


> [!WARNING]
> If you want the asset to be masked, so a part of the mouth or eyebrows is not affected by the tinting, upload a zip file with both the png and the mask files. Remeber that the mask file should have a suffix "\\\_mask" in order to work.


![](</docs/.gitbook/assets/02_mouth_wearable (1).webp>)

After that uploading your wearables you will end up with a screen like this, that shows the items in your collection.

![](</docs/.gitbook/assets/11_list_wearables (1).webp>)

## **Setting the Price of Your Wearables**

Once you publish your collection and it gets approved, you can then enable the sales on your collection.

![](/docs/.gitbook/assets/25_enable_sales.webp)

You can set the price of your wearable by clicking on _**Put up for sale**_. This can all be edited anytime, so don’t worry if you want to change it later on. Prices are set in MANA. Remember that when you mint wearables, they are minted directly on Matic/Polygon. When a user purchases your item, the transaction will be conducted in Matic/Polygon MANA.

You could also _**Make it Free**_, which means that the price will be set as 0 MANA and the beneficiary address will be null. Know that making it free (primary sale) does not prevent it from being sold at any price as a secondary sale.

Don’t forget to set the beneficiary address, which is the one that will receive the MANA from your sales. You can use any Ethereum address you like. To automatically fill in the address you are logged in with, click _**I’m the Beneficiary.**_

![](</docs/.gitbook/assets/12_set_price (1).webp>)

Put the item up for sale and you will be back to the list of wearables in your collection. When you click on the item, you will get its general info. Click on the button _**Preview**_ to see it on the editor.

![](</docs/.gitbook/assets/13_click_item (1).webp>)

## The Editor

Once you click on _**Preview**_, you will have the editor open. You can edit all the info of your wearable, as well as add new ones, such as description, tags and overrides. This also where you add other body shape representation to your wearable.

![](</docs/.gitbook/assets/14_editor (1).webp>)

## **Description**

This is a brief statement describing your item that will be displayed in the marketplace.

## **Overrides**

Overrides determine which Wearable categories or avatar body parts your item will hide. For instance, a hat with attached hair might need to hide the _Hair_ category. A deep-sea diver helmet may require hiding head accessories like earrings, eyewear, tiaras, etc., which wouldn’t be visible. Multiple options can be selected for each override.

* **Base Body**: This refers to core avatar parts like the _head_ and _hands_. For example, if you’re creating a **Handwear** item such as a robot mechanic hand, you’ll likely need to hide _hands_ to prevent overlap and clipping.
* **Wearables**: This includes other Wearable categories. You can hide multiple categories. For more details on each category and how items interact, refer to [**Creating Wearables**](/docs/creator/wearables-and-emotes/wearables/creating-wearables/).


> [!WARNING]
> Note: The overrides you select will be the suggested default settings for your Wearable. However, users can customize which Wearables are hidden or showing from the Backpack.


## **Tags**

Tags are simply descriptive words that users can use when searching or filtering for items. These are relevant to competitions or events!

## **VRM Export Permission**

When this property is enabled, it will allow owners of your item to include it in VRM Avatar Exports so they can show it off outside of Decentraland.

## **Outline**

The Outline toggle controls whether your wearable is compatible with Decentraland's outline rendering system (used in Medium/High quality settings). By default, this is enabled for all new wearables.

You should disable this option if your wearable:

* Has manually created outlines in the model
* Uses inverted normals
* Shows visual artifacts when the outline effect is applied

Most wearables work fine with the default setting (enabled). Only disable this if you notice outline-related visual issues with your wearable. Once you disable the outline, you will need to save the changes by clicking on _**Save**_.

![](/docs/.gitbook/assets/26_outline.webp)

## **Adding Another Representation**

If your wearable has a different representation for male and female you will need to upload another file. So far, you only have one uploaded. In the example, it was the female version of the Krampus Sweater. To add the other representation, click on the three dots (_…_) at the top right, next to _**Properties**_ and select _**Add male/female**_ representation. In the example below, we needed the male version.

![](/docs/.gitbook/assets/15_add_variation.webp)

Once you click on it, you will get the Add Male/Female representation window, drag and drop the other representation file to upload it. Once uploaded, another window will show up the wearable and if everything is ok, just click on Save.

![](/docs/.gitbook/assets/16_add_male_representation_01.webp) ![](/docs/.gitbook/assets/17_add_male_representation_02.webp)

## **Preview**

To preview your wearable, hover your mouse over the wearable icon on the top left and click on the eye symbol.

![](/docs/.gitbook/assets/18_preview_wearables_03.webp)

By clicking on the icon at the lower left you will be able to edit the avatar. This is pretty useful if you have a male and female version of your wearable, so make sure to check how both versions look like in editor, testing different emotes to identify if there are any skinning issues and mixing other wearables to see how it matches with different clothes. When you’re done editing your wearable, click on _**Save**_.

![](</docs/.gitbook/assets/19_edit_avatar_wearable_03 (1).webp>)

## **Testing in World**

Even after testing the wearable in the editor, it’s important to check how it’s actually going to look like and behave in Decentraland. To test it in world, go to the Collections tab. Select the desired collection and click the button _**See in World**_.

![](</docs/.gitbook/assets/20_test_in_world_01 (1).webp>)

After clicking the following pop up is going to appear. Selecting _**Empty Parcels**_ will teleport you to a place without too much content, which will load faster. Selecting _**Genesis Plaza**_ will take you to the main plaza.

![](</docs/.gitbook/assets/23_see_in_world (1).webp>)

Once you select See in world, a new tab will open on your browser, and you will get this message.

![](/docs/.gitbook/assets/27_wearable_preview.webp)

Click on _**TRUST PEER-TESTING.DECENTRALAND.ORG**_ and a pop-up will show up. Simply click on Open Decentraland. To test your wearable, go to the backpack and equip it.

![](/docs/.gitbook/assets/28_wearable_world.webp)

## **Before Publishing**

Make sure to set the price properly, add a nice description and double check if all the information and settings are right. If you’ve filled all the information necessary you will see _**Done**_ as the status of your item.
